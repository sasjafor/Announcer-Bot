mod commands;
mod util;

use std::{
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
    all::{ClientBuilder, ShardManager}, async_trait, client::{Context, EventHandler}, model::{
        event::ResumedEvent,
        gateway::Ready,
        id::UserId,
        prelude::*,
        voice::VoiceState,
    }, prelude::{Mutex, TypeMapKey}
};

use rusqlite::{params, Connection};
use tracing::{debug, error, info};

use commands::{list::*, new::*, random::*, set::*, names::*};

use util::{check::can_connect, util::send_debug};

use crate::util::{
    consts::{BOT_ADMIN_USER_ID, CUZ_USER_ID}, 
    util::{announce, bot_voice_channel_is_empty, leave_channel, play_file, print_type_of}
};

// Types used by all command functions
type PError = Box<dyn std::error::Error + Send + Sync>;
type PContext<'a> = poise::Context<'a, Data, PError>;

// Custom user data passed to all command functions
pub struct Data {}

struct ShardManagerContainer;

impl TypeMapKey for ShardManagerContainer {
    type Value = Arc<Mutex<ShardManager>>;
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _ctx: Context, ready: Ready) {
        info!("Connected as {}", ready.user.name);
    }

    async fn resume(&self, _: Context, _: ResumedEvent) {
        info!("Resumed");
    }

    async fn voice_state_update(&self, ctx: Context, old_state_opt: Option<VoiceState>, new_state: VoiceState) {
        const USER1_ID: UserId = UserId::new(CUZ_USER_ID);
        const USER2_ID: UserId = UserId::new(BOT_ADMIN_USER_ID);

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

        let maybe_channel_id = new_state.channel_id;
        let new_channel_exists = maybe_channel_id.is_some();
        let cant_connect = !can_connect(&ctx, maybe_channel_id);

        if let Some(old_state) = &old_state_opt {
            if !new_channel_exists ||
               (new_channel_exists &&
               (Some(guild_id) != old_state.guild_id ||
                cant_connect)) {

                if let Some(prev_guild_id) = old_state.guild_id {
                    if bot_voice_channel_is_empty(&ctx, prev_guild_id).await {
                        leave_channel(&ctx, prev_guild_id).await;
                        info!("Left empty voice channel");
                        return;
                    }
                }
            }
        }

        if !new_channel_exists {
            debug!("New channel doesn't exist.");
            return;
        }
        let channel_id = maybe_channel_id.unwrap();

        if cant_connect {
            debug!("Not allowed to connect to new channel.");
            return;
        }

        if !(&old_state_opt).is_some() {
            let path = "/config/StGallerConnection.flac";
            if user_id == USER1_ID {
                let user_check;
                {
                    let guild = guild_id.to_guild_cached(&ctx).unwrap();
                    user_check = match guild.voice_states.get(&USER2_ID) {
                        Some(user_check) => user_check.channel_id == Some(channel_id),
                        None => false
                    };
                }

                if user_check {
                    let _ = play_file(&ctx, channel_id, guild_id, &path).await;
                }
            }

            if user_id == USER2_ID {
                let user_check;
                {
                    let guild = guild_id.to_guild_cached(&ctx).unwrap();
                    user_check = match guild.voice_states.get(&USER1_ID) {
                        Some(user_check) => user_check.channel_id == Some(channel_id),
                        None => false
                    };
                }

                if user_check {
                    let _ = play_file(&ctx, channel_id, guild_id, &path).await;
                }
            }
        }

        if ((&old_state_opt).is_none() && !new_state.self_mute) || 
           ((&old_state_opt).is_some() && 
            (old_state_opt.as_ref().unwrap().self_mute || old_state_opt.as_ref().unwrap().channel_id.unwrap() != channel_id) && 
             !new_state.self_mute) {
            info!("UNMUTE!");

            let member = guild_id.member(&ctx.http, user_id).await.unwrap();

            let name = member.display_name().to_string().replace("/", "⁄");

            let _ = announce(&ctx, channel_id, guild_id, &name, user_id.get()).await;
        }
    }
}

/// Show this help menu
#[poise::command(prefix_command, track_edits, slash_command)]
async fn help(
    ctx: PContext<'_>,
    #[description = "Specific command to show help about"]
    #[autocomplete = "poise::builtins::autocomplete_command"]
    command: Option<String>,
) -> Result<(), PError> {
    poise::builtins::help(
        ctx,
        command.as_deref(),
        poise::builtins::HelpConfiguration {
            extra_text_at_bottom: format!(
                "If you have questions just ask {}",
                UserId::new(180995420196044809).mention()
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
async fn register(ctx: PContext<'_>) -> Result<(), PError> {
    poise::builtins::register_application_commands_buttons(ctx).await?;

    Ok(())
}

async fn on_error(error: poise::FrameworkError<'_, Data, PError>) {
    // This is our custom error handler
    // They are many errors that can occur, so we only handle the ones we want to customize
    // and forward the rest to the default handler
    match error {
        poise::FrameworkError::Setup { error, .. } => panic!("Failed to start bot: {:?}", error),
        poise::FrameworkError::Command { error, ctx, .. } => {
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
    let token = env::var("DISCORD_APP_AUTH_TOKEN").expect("Expected `DISCORD_APP_AUTH_TOKEN` in the environment");

    let intents = GatewayIntents::GUILD_MEMBERS
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILD_VOICE_STATES
        | GatewayIntents::GUILDS
        | GatewayIntents::MESSAGE_CONTENT;

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                help(),
                register(),
                set(),
                list(),
                new(),
                random(),
                names(),
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
                        739933045406171166, // announcer-bot-submissions (Rütlischwur Dudes)
                        955573958403571822, // announcer-bot-submissions (Spielbande)
                    ]
                    .contains(&ctx.channel_id().get())
                    {
                        return Ok(true);
                    } else {
                        let why = "Channel not in allowed list.";
                        let err_string = format!("Use a valid channel to send commands");
                        let _ = send_debug(ctx, err_string, why.to_string()).await;
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
        .setup(move |_ctx, _ready, _framework| {
            Box::pin(async move {
                Ok(Data {})
            })
        })
        .build();

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

    let client = ClientBuilder::new(token, intents)
        .event_handler(Handler)
        .framework(framework)
        .register_songbird()
        .await;

    client.unwrap()
        .start()
        .await
        .unwrap();
}
