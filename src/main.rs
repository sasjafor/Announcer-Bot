mod commands;
mod lib;

use std::{
    collections::HashSet,
    env,
    fs::{self},
    path::Path,
    sync::Arc,
};

use regex::Regex;
// This trait adds the `register_songbird` and `register_songbird_with` methods
// to the client builder below, making it easy to install this voice client.
// The voice client can be retrieved in any command using `songbird::get(ctx).await`.
use songbird::SerenityInit;

use serenity::{
    async_trait,
    client::{bridge::gateway::ShardManager, Client, Context, EventHandler},
    framework::{
        standard::{
            help_commands,
            macros::{group, help},
            Args, CommandGroup, CommandResult, HelpOptions,
        },
        StandardFramework,
    },
    http::Http,
    model::{
        event::ResumedEvent,
        gateway::Ready,
        id::{ChannelId, UserId},
        interactions::{
            application_command::{
                ApplicationCommand, ApplicationCommandInteractionDataOptionValue,
            },
            message_component::ComponentType,
        },
        prelude::*,
        voice::VoiceState,
    },
    prelude::{Mutex, TypeMapKey},
};

use tracing::{debug, error, info};

use rusqlite::{params, Connection};

use commands::{list::*, newfile::*, random::*, set::*};

use lib::check::can_connect;

use crate::lib::util::{announce, play_file, print_type_of, voice_channel_is_empty};

struct ShardManagerContainer;

impl TypeMapKey for ShardManagerContainer {
    type Value = Arc<Mutex<ShardManager>>;
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            debug!("Received slash command: {:#?}", command.data.name);

            let (content, embed, components) = match command.data.name.as_str() {
                "list" => {
                    let options = &command.data.options;

                    let mut name_arg: Option<String> = None;
                    let mut index_arg: Option<usize> = None;
                    for arg in options {
                        let arg_val = arg.resolved.as_ref().expect("Expected object").to_owned();
                        match arg.name.as_str() {
                            "name" => {
                                if let ApplicationCommandInteractionDataOptionValue::String(
                                    string_val,
                                ) = arg_val
                                {
                                    name_arg = Some(string_val);
                                }
                            }
                            "index" => {
                                if let ApplicationCommandInteractionDataOptionValue::Integer(
                                    int_val,
                                ) = arg_val
                                {
                                    index_arg = Some(int_val as usize);
                                }
                            }
                            _ => (),
                        }
                    }

                    let name = match name_arg {
                        Some(name_val) => name_val,
                        None => {
                            if let Some(member) = command.member.clone() {
                                member.display_name().into_owned()
                            } else {
                                command.user.name.clone()
                            }
                        }
                    };
                    let index = index_arg.unwrap_or(1);

                    list(&ctx, &command.user, &name, index).await
                }
                _ => ("Command not implemented".to_string(), None, None),
            };

            if let Err(why) = command
                .create_interaction_response(&ctx.http, |response| {
                    response
                        .kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|message| {
                            message.content(content);
                            if let Some(e) = embed {
                                message.add_embed(e);
                            }
                            if let Some(c) = components {
                                message.set_components(c);
                            }
                            message
                        })
                })
                .await
            {
                error!("Couldn't respond to slash command: {}", why);
            }
        } else if let Interaction::MessageComponent(mut component) = interaction {
            debug!("Received message component interaction");

            let footer_text = component
                .message
                .embeds
                .get(0)
                .expect("Embed is missing.")
                .footer
                .as_ref()
                .expect("Footer is missing from list embed.")
                .text
                .clone();

            let re_idx = Regex::new(r"Page ([0-9]+)/[0-9]+").unwrap();
            let re_match = re_idx
                .captures(&footer_text)
                .expect("Couldn't match page index with regex.")
                .get(1)
                .expect("No matches.")
                .as_str();
            let mut index = re_match.parse::<usize>().expect("Failed to parse int.");
            index = match component.data.component_type {
                ComponentType::Button => match component.data.custom_id.as_str() {
                    "Prev Button" => index - 1,
                    "Next Button" => index + 1,
                    _ => index,
                },
                _ => index,
            };

            let title_text = component
                .message
                .embeds
                .get(0)
                .expect("Embed is missing.")
                .title
                .as_ref()
                .expect("Title is missing from list embed.")
                .clone();
            let re_name = Regex::new(r#"Announcements for "(.+)""#).unwrap();
            let name = re_name
                .captures(&title_text)
                .expect("Couldn't match announcement name with regex.")
                .get(1)
                .expect("No matches.")
                .as_str()
                .to_owned();

            let (content, embed, components) = list(&ctx, &component.user, &name, index).await;

            if let Err(why) = component
                .create_interaction_response(&ctx.http, |response| {
                    response
                        .kind(InteractionResponseType::UpdateMessage)
                        .interaction_response_data(|message| {
                            message.content(content);
                            if let Some(e) = embed {
                                message.set_embed(e);
                            }
                            if let Some(c) = components {
                                message.set_components(c);
                            }
                            message
                        })
                })
                .await
            {
                error!("Couldn't edit message: {}", why);
            };
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("Connected as {}", ready.user.name);

        let guild_command =
            ApplicationCommand::create_global_application_command(&ctx.http, create_list_command)
                .await;

        if let Ok(gc) = guild_command {
            debug!("Created global slash command: {:#?}", gc.name);
        } else {
            error!(
                "Failed to create global slash command: {:?}",
                guild_command.err()
            );
        }
    }

    async fn resume(&self, _: Context, _: ResumedEvent) {
        info!("Resumed");
    }

    async fn voice_state_update(
        &self,
        ctx: Context,
        old_state: Option<VoiceState>,
        new_state: VoiceState,
    ) {
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
                        if voice_channel_is_empty(
                            &ctx,
                            &guild,
                            ChannelId(self_channel_id.unwrap().0),
                        )
                        .await
                        {
                            let _ = handler
                                .leave()
                                .await
                                .expect("Failed to leave voice channel");
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

#[group]
#[commands(newfile, set, random)]
#[only_in("guilds")]
#[help_available]
struct General;

#[help]
#[no_help_available_text("No help available for this command")]
#[command_not_found_text = "Could not find: `{}`."]
#[max_levenshtein_distance(3)]
async fn my_help(
    context: &Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,
) -> CommandResult {
    let _ = help_commands::plain(context, msg, args, help_options, groups, owners).await;
    Ok(())
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

    let http = Http::new(&token);

    // We will fetch your bot's owners and id
    let (_owners, _bot_id) = match http.get_current_application_info().await {
        Ok(info) => {
            let mut owners = HashSet::new();
            owners.insert(info.owner.id);

            (owners, info.id)
        }
        Err(why) => panic!("Could not access application info: {:?}", why),
    };

    // Create the framework
    let framework = StandardFramework::new()
        .group(&GENERAL_GROUP)
        .help(&MY_HELP)
        .configure(|c| {
            c.prefix("!")
                .allow_dm(false)
                .case_insensitivity(true)
                .allowed_channels(
                    vec![
                        ChannelId(552168558323564544), // announcer-bot-submissions (Test server)
                        ChannelId(511144158975623169), // announcer-bot-submissions (Cupboard under the stairs)
                        ChannelId(780475875698409502), // test channel
                        ChannelId(739933045406171166), // gay-announcement (Rütlischwur Dudes)
                        ChannelId(955573958403571822), // announcer-bot-submissions (Spielbande)
                    ]
                    .into_iter()
                    .collect(),
                )
        });

    let intents = GatewayIntents::GUILD_MEMBERS
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILD_VOICE_STATES
        | GatewayIntents::GUILDS;
    let mut client = Client::builder(&token, intents)
        .framework(framework)
        .event_handler(Handler)
        .register_songbird()
        .await
        .expect("Err creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<ShardManagerContainer>(client.shard_manager.clone());
    }

    let shard_manager = client.shard_manager.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Could not register ctrl+c handler");
        shard_manager.lock().await.shutdown_all().await;
    });

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

    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        error!("Client error: {:?}", why);
    }
}
