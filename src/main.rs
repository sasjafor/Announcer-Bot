mod lib;
mod commands;

use std::{
    collections::HashSet,
    env, 
    fs::{
        self,
    }, 
    path::Path, 
    sync::{
        Arc,
    },
};

// This trait adds the `register_songbird` and `register_songbird_with` methods
// to the client builder below, making it easy to install this voice client.
// The voice client can be retrieved in any command using `songbird::get(ctx).await`.
use songbird::SerenityInit;

use serenity::{
    async_trait,
    client::{
        bridge::{
            gateway::{
                ShardManager,
            }
        },
        Client, 
        Context,
        EventHandler
    },
    framework::{
        standard::{
            Args,
            CommandGroup,
            CommandResult,
            HelpOptions,
            help_commands,
            macros::{
                group,
                help,
            }, 
        },
        StandardFramework
    },
    http::Http,
    model::{
        event::ResumedEvent, 
        gateway::Ready, 
        id::{
            ChannelId,
            UserId,
        }, 
        prelude::*,
        voice::VoiceState,
    },
    prelude::{
        TypeMapKey,
        Mutex,
    },
};

use tracing::{error, info};
use tracing_subscriber::{
    FmtSubscriber,
};

use rusqlite::{
    Connection,
    params,
};

use commands::{
    newfile::*,
    list::*,
};

use lib::check::can_connect;

use crate::lib::util::{voice_channel_is_empty, play_file, print_type_of, announce};

struct ShardManagerContainer;

impl TypeMapKey for ShardManagerContainer {
    type Value = Arc<Mutex<ShardManager>>;
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
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
                let manager = songbird::get(&ctx).await
                    .expect("Songbird Voice client placed in at initialisation.").clone();
    
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

#[group]
#[commands(newfile, list, set, random)]
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
    owners: HashSet<UserId>
) -> CommandResult {
    let _ = help_commands::plain(context, msg, args, help_options, groups, owners).await;
    Ok(())
}

#[tokio::main]
async fn main() {
    // Initialize the logger to use environment variables.
    //
    // In this case, a good default is setting the environment variable
    // `RUST_LOG` to debug`.
    let subscriber = FmtSubscriber::builder()
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("Failed to start the logger");

    // Login with a bot token from the environment
    let token = env::var("DISCORD_APP_AUTH_TOKEN").expect("Expected a token in the environment");

    let http = Http::new(&token);

    // We will fetch your bot's owners and id
    let (_owners, _bot_id) = match http.get_current_application_info().await {
        Ok(info) => {
            let mut owners = HashSet::new();
            owners.insert(info.owner.id);

            (owners, info.id)
        },
        Err(why) => panic!("Could not access application info: {:?}", why),
    };

    // Create the framework
    let framework = StandardFramework::new()
        .group(&GENERAL_GROUP)
            .help(&MY_HELP)
            .configure(|c| c
                .prefix("!")
                .allow_dm(false)
                .case_insensitivity(true)
                .allowed_channels(vec![ ChannelId(552168558323564544), // announcer-bot-submissions (Test server)
                                        ChannelId(511144158975623169), // announcer-bot-submissions (Cupboard under the stairs)
                                        ChannelId(780475875698409502), // test channel
                                        ChannelId(739933045406171166), // gay-announcement (Rütlischwur Dudes)
                                        ChannelId(955573958403571822)  // announcer-bot-submissions (Spielbande)
                                        ].into_iter().collect())
            );

    let intents =   GatewayIntents::GUILD_MEMBERS |
                                    GatewayIntents::GUILD_MESSAGES |
                                    GatewayIntents::GUILD_VOICE_STATES |
                                    GatewayIntents::GUILDS;
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
        tokio::signal::ctrl_c().await.expect("Could not register ctrl+c handler");
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
        params![]) {
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
