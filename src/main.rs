mod commands;
mod lib;

use std::{
    collections::HashMap,
    env,
    fs::{self},
    path::Path,
    sync::Arc,
};

// This trait adds the `register_songbird` and `register_songbird_with` methods
// to the client builder below, making it easy to install this voice client.
// The voice client can be retrieved in any command using `songbird::get(ctx).await`.
use songbird::SerenityInit;

use serenity::{
    async_trait,
    client::{bridge::gateway::ShardManager, Context, EventHandler},
    model::{
        event::ResumedEvent,
        gateway::Ready,
        id::{ChannelId, UserId},
        interactions::application_command::{
            ApplicationCommand, ApplicationCommandInteractionDataOptionValue, ApplicationCommandOptionType,
        },
        prelude::*,
        voice::VoiceState,
    },
    prelude::{Mutex, TypeMapKey},
};

use rusqlite::{params, Connection};
use tracing::{debug, error, info, warn};

use commands::{list::*, new::*, random::*, set::*};

use lib::check::can_connect;

use crate::lib::util::{announce, play_file, print_type_of, voice_channel_is_empty};

// Types used by all command functions
type Error = Box<dyn std::error::Error + Send + Sync>;
type pContext<'a> = poise::Context<'a, Data, Error>;

// Custom user data passed to all command functions
pub struct Data {}

struct ShardManagerContainer;

impl TypeMapKey for ShardManagerContainer {
    type Value = Arc<Mutex<ShardManager>>;
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            let (content, embed, components) = match command.data.name.as_str() {
                "new" => {
                    let command_option = command.data.options.get(0).expect("Expected command option.");

                    if command_option.kind != ApplicationCommandOptionType::SubCommand {
                        error!("Expected sub command.");
                    }

                    let mut res = ("Command not implemented".to_string(), None, None);

                    let user_option = command_option
                        .options
                        .get(0)
                        .expect("Expected user.")
                        .resolved
                        .as_ref()
                        .expect("Expected user obj.");
                    if let ApplicationCommandInteractionDataOptionValue::User(user, member) = user_option {
                        let announcement_option = command_option
                            .options
                            .get(1)
                            .expect("Expected announcement.")
                            .resolved
                            .as_ref()
                            .expect("Expected announcement obj.");
                        if let ApplicationCommandInteractionDataOptionValue::String(announcement) = announcement_option
                        {
                            if command_option.name == "file" {
                                let attachment_option = command_option
                                    .options
                                    .get(2)
                                    .expect("Expected attachment.")
                                    .resolved
                                    .as_ref()
                                    .expect("Expected attachment obj.");
                                if let ApplicationCommandInteractionDataOptionValue::Attachment(attachment) =
                                    attachment_option
                                {
                                    let filters = match command_option.options.get(3) {
                                        Some(filters) => {
                                            let filters_option =
                                                filters.resolved.as_ref().expect("Expected filters obj.");
                                            if let ApplicationCommandInteractionDataOptionValue::String(filters) =
                                                filters_option
                                            {
                                                Some(filters)
                                            } else {
                                                None
                                            }
                                        }
                                        None => None,
                                    };

                                    let name = match member {
                                        Some(member) => match &member.nick {
                                            Some(nick) => nick.clone(),
                                            None => user.name.clone(),
                                        },
                                        None => user.name.clone(),
                                    };
                                    res = new_file(&ctx, &name, announcement, attachment, user, filters).await;
                                }
                            } else if command_option.name == "url" {
                                let url_option = command_option
                                    .options
                                    .get(2)
                                    .expect("Expected url.")
                                    .resolved
                                    .as_ref()
                                    .expect("Expected url obj.");
                                if let ApplicationCommandInteractionDataOptionValue::String(url) = url_option {
                                    let start_option = command_option
                                        .options
                                        .get(3)
                                        .expect("Expected start.")
                                        .resolved
                                        .as_ref()
                                        .expect("Expected start obj.");
                                    if let ApplicationCommandInteractionDataOptionValue::String(start) = start_option {
                                        let end_option = command_option
                                            .options
                                            .get(4)
                                            .expect("Expected start.")
                                            .resolved
                                            .as_ref()
                                            .expect("Expected start obj.");
                                        if let ApplicationCommandInteractionDataOptionValue::String(end) = end_option {
                                            let filters = match command_option.options.get(5) {
                                                Some(filters) => {
                                                    let filters_option =
                                                        filters.resolved.as_ref().expect("Expected filters obj.");
                                                    if let ApplicationCommandInteractionDataOptionValue::String(
                                                        filters,
                                                    ) = filters_option
                                                    {
                                                        Some(filters)
                                                    } else {
                                                        None
                                                    }
                                                }
                                                None => None,
                                            };

                                            let name = match member {
                                                Some(member) => match &member.nick {
                                                    Some(nick) => nick.clone(),
                                                    None => user.name.clone(),
                                                },
                                                None => user.name.clone(),
                                            };
                                            res = new_url(&ctx, &name, announcement, url, start, end, user, filters)
                                                .await;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    res
                }
                _ => ("".to_string(), None, None),
            };
        } else if let Interaction::MessageComponent(component) = interaction {
            // debug!("Received message component interaction");

            // let footer_text = component
            //     .message
            //     .embeds
            //     .get(0)
            //     .expect("Embed is missing.")
            //     .footer
            //     .as_ref()
            //     .expect("Footer is missing from list embed.")
            //     .text
            //     .clone();

            // let re_idx = Regex::new(r"Page ([0-9]+)/[0-9]+").unwrap();
            // let re_match = re_idx
            //     .captures(&footer_text)
            //     .expect("Couldn't match page index with regex.")
            //     .get(1)
            //     .expect("No matches.")
            //     .as_str();
            // let mut index = re_match.parse::<usize>().expect("Failed to parse int.");
            // index = match component.data.component_type {
            //     ComponentType::Button => match component.data.custom_id.as_str() {
            //         "Prev Button" => index - 1,
            //         "Next Button" => index + 1,
            //         _ => index,
            //     },
            //     _ => index,
            // };

            // let title_text = component
            //     .message
            //     .embeds
            //     .get(0)
            //     .expect("Embed is missing.")
            //     .title
            //     .as_ref()
            //     .expect("Title is missing from list embed.")
            //     .clone();
            // let re_name = Regex::new(r#"Announcements for "(.+)""#).unwrap();
            // let name = re_name
            //     .captures(&title_text)
            //     .expect("Couldn't match announcement name with regex.")
            //     .get(1)
            //     .expect("No matches.")
            //     .as_str()
            //     .to_owned();

            // let (content, embed, components) = list(&ctx, &component.user, &name, index).await;

            // if let Err(why) = component
            //     .create_interaction_response(&ctx.http, |response| {
            //         response
            //             .kind(InteractionResponseType::UpdateMessage)
            //             .interaction_response_data(|message| {
            //                 message.content(content);
            //                 if let Some(e) = embed {
            //                     message.set_embed(e);
            //                 }
            //                 if let Some(c) = components {
            //                     message.set_components(c);
            //                 }
            //                 message
            //             })
            //     })
            //     .await
            // {
            //     error!("Couldn't edit message: {}", why);
            // };
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("Connected as {}", ready.user.name);
    }

    async fn resume(&self, _: Context, _: ResumedEvent) {
        info!("Resumed");
    }

    async fn voice_state_update(&self, ctx: Context, old_state: Option<VoiceState>, new_state: VoiceState) {
        const USER1_ID: UserId = UserId(239705630913331201); // demain
        const USER2_ID: UserId = UserId(180995420196044809); // seschu

        let user_id = new_state.user_id;

        let user = match user_id.to_user(&ctx).await {
            Ok(user) => user,
            Err(e) => {
                error!("User not found: {:?}", e);
                return;
            }
        };

        let is_bot = user.bot;
        if is_bot {
            return;
        }

        let guild_id = match new_state.guild_id {
            Some(guild_id) => guild_id,
            None => {
                info!("Guild id not found.");
                return;
            }
        };

        let guild = match guild_id.to_guild_cached(&ctx) {
            Some(guild) => guild,
            None => {
                info!("Guild not found in cache.");
                return;
            }
        };

        let maybe_channel_id = new_state.channel_id;

        if (&old_state).is_some() {
            if maybe_channel_id.is_none() || !can_connect(&ctx, maybe_channel_id.unwrap()).await {
                let manager = songbird::get(&ctx)
                    .await
                    .expect("Songbird Voice client placed in at initialisation.")
                    .clone();

                let handler_lock = manager.get(guild_id);

                if handler_lock.is_some() {
                    let handler_tmp = handler_lock.unwrap();
                    let mut handler = handler_tmp.lock().await;

                    let self_channel_id = handler.current_channel();

                    if self_channel_id.is_some() {
                        if voice_channel_is_empty(&ctx, &guild, ChannelId(self_channel_id.unwrap().0)).await {
                            let _ = handler.leave().await.expect("Failed to leave voice channel");
                            info!("Left empty voice channel");
                            return;
                        }
                    }
                }
                return;
            }
        } else {
            if maybe_channel_id.is_none() || !can_connect(&ctx, maybe_channel_id.unwrap()).await {
                return;
            }

            let channel_id = maybe_channel_id.unwrap();

            let path = "/config/StGallerConnection.mp3";
            if user_id == USER1_ID {
                let user_check = guild.voice_states.get(&USER2_ID);

                if user_check.is_some() && user_check.unwrap().channel_id == Some(channel_id) {
                    let _ = play_file(&ctx, channel_id, guild_id, &path).await;
                }
            }

            if user_id == USER2_ID {
                let user_check = guild.voice_states.get(&USER1_ID);

                if user_check.is_some() && user_check.unwrap().channel_id == Some(channel_id) {
                    let _ = play_file(&ctx, channel_id, guild_id, &path).await;
                }
            }
        }

        let channel_id = maybe_channel_id.unwrap();

        if !new_state.self_mute {
            info!("UNMUTE!");

            let member = match guild_id.member(&ctx.http, user_id).await {
                Ok(member) => member,
                Err(e) => {
                    error!("Member not found: {:?}", e);
                    return;
                }
            };

            let name = member.display_name().to_string().replace("/", "⁄");

            let _ = announce(&ctx, channel_id, guild_id, &name, user_id.0).await;
            return;
        }
    }
}

/// Show this help menu
#[poise::command(prefix_command, track_edits, slash_command)]
async fn help(
    ctx: pContext<'_>,
    #[description = "Specific command to show help about"]
    #[autocomplete = "poise::builtins::autocomplete_command"]
    command: Option<String>,
) -> Result<(), Error> {
    poise::builtins::help(
        ctx,
        command.as_deref(),
        poise::builtins::HelpConfiguration {
            extra_text_at_bottom: format!(
                "If you have questions just ask {}",
                UserId(180995420196044809).mention()
            )
            .as_str(),
            show_context_menu_commands: true,
            ..Default::default()
        },
    )
    .await?;
    Ok(())
}

/// Registers or unregisters application commands in this guild or globally
#[poise::command(prefix_command, hide_in_help)]
async fn register(ctx: pContext<'_>) -> Result<(), Error> {
    poise::builtins::register_application_commands_buttons(ctx).await?;

    Ok(())
}

async fn on_error(error: poise::FrameworkError<'_, Data, Error>) {
    // This is our custom error handler
    // They are many errors that can occur, so we only handle the ones we want to customize
    // and forward the rest to the default handler
    match error {
        poise::FrameworkError::Setup { error } => panic!("Failed to start bot: {:?}", error),
        poise::FrameworkError::Command { error, ctx } => {
            println!("Error in command `{}`: {:?}", ctx.command().name, error,);
        }
        error => {
            if let Err(e) = poise::builtins::on_error(error).await {
                println!("Error while handling error: {}", e)
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Call tracing_subscriber's initialize function, which configures `tracing`
    // via environment variables.
    //
    // For example, you can say to log all levels INFO and up via setting the
    // environment variable `RUST_LOG` to `INFO`.
    //
    // This environment variable is already preset if you use cargo-make to run
    // the example.
    tracing_subscriber::fmt::init();

    // Login with a bot token from the environment
    let token = env::var("DISCORD_APP_AUTH_TOKEN").expect("Expected a token in the environment");

    let intents = GatewayIntents::GUILD_MEMBERS
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILD_VOICE_STATES
        | GatewayIntents::GUILDS
        | GatewayIntents::MESSAGE_CONTENT;
    let framework = poise::Framework::build()
        .options(poise::FrameworkOptions {
            commands: vec![
                help(),
                register(),
                set(),
                list(),
                // new()
            ],
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some("!".into()),
                mention_as_prefix: true,
                case_insensitive_commands: true,
                ..Default::default()
            },
            on_error: |error| Box::pin(on_error(error)),
            command_check: Some(|ctx| {
                Box::pin(async move {
                    if [
                        552168558323564544, // announcer-bot-submissions (Test server)
                        511144158975623169, // announcer-bot-submissions (Cupboard under the stairs)
                        780475875698409502, // test channel
                        739933045406171166, // gay-announcement (Rütlischwur Dudes)
                        955573958403571822, // announcer-bot-submissions (Spielbande)
                    ]
                    .contains(&ctx.channel_id().0)
                    {
                        return Ok(true);
                    } else {
                        return Ok(false);
                    }
                })
            }),
            pre_command: |ctx| {
                Box::pin(async move {
                    debug!("Executing command {}", ctx.command().qualified_name);

                    if let Err(why) = ctx.defer_or_broadcast().await {
                        error!("Couldn't respond to command: {}", why);
                    }
                })
            },
            ..Default::default()
        })
        .token(token)
        .intents(intents)
        .user_data_setup(move |_ctx, _ready, _framework| Box::pin(async move { Ok(Data {}) }))
        .client_settings(move |f| f.register_songbird().event_handler(Handler));

    let audio = Path::new("/config/audio");
    let index = Path::new("/config/index");
    let queue = Path::new("/config/queue");
    let processing = Path::new("/config/processing/");
    let db_folder = Path::new("/config/database/");
    let db_path = Path::new("/config/database/db.sqlite");

    if !audio.exists() {
        let _ = fs::create_dir(audio);
    }

    if !index.exists() {
        let _ = fs::create_dir(index);
    }

    if !queue.exists() {
        let _ = fs::create_dir(queue);
    }

    if !processing.exists() {
        let _ = fs::create_dir(processing);
    }

    if !db_folder.exists() {
        let _ = fs::create_dir(db_folder);
    }

    let db = match Connection::open(&db_path) {
        Ok(db) => db,
        Err(err) => {
            error!("Failed to open database: {}", err);
            return;
        }
    };

    let _ = match db.execute(
        "CREATE TABLE IF NOT EXISTS names (
            name            TEXT NOT NULL, 
            user_id         INTEGER NOT NULL,
            active_file     TEXT NOT NULL DEFAULT '',
            random          INTEGER NOT NULL DEFAULT 0 CHECK(random IN(0, 1)),
            PRIMARY KEY ( name, user_id )
            )",
        params![],
    ) {
        Ok(_) => (),
        Err(err) => {
            print_type_of(&err);
            error!("Failed to create table, Error Code: {}", err);
            return;
        }
    };

    // currently this is just running without shards, which I think is fine
    framework.run().await.unwrap();
}
