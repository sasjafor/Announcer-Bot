#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;

extern crate env_logger;
extern crate serenity;
extern crate url;
extern crate regex;
extern crate rusqlite;
extern crate rand;

mod lib;
mod commands;

use std::{
    collections::HashSet,
    env, 
    fs::{
        self,
        read_dir,
    }, 
    path::Path, 
    process::Command, 
    sync::{
        Arc,
    },
};

use serenity::{
    client::{
        bridge::voice::ClientVoiceManager,
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
    model::{
        event::ResumedEvent, 
        gateway::Ready, 
        guild::Guild, 
        id::{
            ChannelId,
            GuildId,
            UserId,
        }, 
        prelude::*,
        voice::VoiceState,
    },
    prelude::{
        TypeMapKey,
        Mutex,
    },
    voice,
};

use commands::{
    newfile::*,
    manage::*,
};

use rusqlite::{
    Connection,
    OptionalExtension,
    params,
};

use rand::Rng;

struct VoiceManager;

impl TypeMapKey for VoiceManager {
    type Value = Arc<Mutex<ClientVoiceManager>>;
}

struct Handler;

impl EventHandler for Handler {
    fn ready(&self, _: Context, ready: Ready) {
        info!("Connected as {}", ready.user.name);
    }

    fn resume(&self, _: Context, _: ResumedEvent) {
        info!("Resumed");
    }

    fn voice_state_update(&self, ctx: Context, guild_id: Option<GuildId>, old_state: Option<VoiceState>, new_state: VoiceState) {
        const USER1_ID: UserId = UserId(239705630913331201);
        const USER2_ID: UserId = UserId(180995420196044809);

        let user_id = new_state.user_id;

        let user = match user_id.to_user(&ctx) {
            Ok(user) => user,
            Err(e) => {
                error!("User not found: {:?}", e);
                return;
            }
        };

        let guild_id = match guild_id {
            Some(guild_id) => guild_id,
            None => {
                info!("Guild id not found.");
                return;
            }
        };

        let guild = match guild_id.to_guild_cached(&ctx) {
            Some(guild) => guild.read().clone(),
            None => {
                info!("Guild not found in cache.");
                return;
            }
        };

        let maybe_channel_id = new_state.channel_id;

        if (&old_state).is_some() && maybe_channel_id.is_none() {
            let data = &ctx.data.read();
            let manager_lock = data
                .get::<VoiceManager>()
                .cloned()
                .expect("Expected VoiceManager in ShareMap.");

            let mut manager = manager_lock.lock();
            let maybe_handler = manager.get_mut(guild_id);

            if maybe_handler.is_some() {
                let handler = maybe_handler.unwrap();

                let self_channel_id = handler.channel_id;

                if self_channel_id.is_some() {
                    if voice_channel_is_empty(&ctx, &guild, self_channel_id.unwrap()) {
                        info!("Voice channel empty, leaving...");
                        handler.leave();
                        return;
                    }
                }
            }
        } 

        
        if maybe_channel_id.is_none() {
            return;
        }
        let channel_id = maybe_channel_id.unwrap();
        
        if (&old_state).is_none() {
            let path = "/config/StGallerConnection.mp3";
            if user_id == USER1_ID {
                let mut user_check = guild
                                    .voice_states
                                    .values()
                                    .filter(|state| state.channel_id == Some(channel_id))
                                    .filter(|state| state.user_id == USER2_ID)
                                    .peekable();
                    
                if user_check.peek().is_some() {
                    
                    play_file(&ctx, channel_id, guild_id, &path);
                }
            }

            if user_id == USER2_ID {
                let mut user_check = guild
                                    .voice_states
                                    .values()
                                    .filter(|state| state.channel_id == Some(channel_id))
                                    .filter(|state| state.user_id == USER1_ID)
                                    .peekable();
                    
                if user_check.peek().is_some() {
                    play_file(&ctx, channel_id, guild_id, &path);
                }
            }
        }

        let is_bot = user.bot;

        if !is_bot && !new_state.self_mute {
            info!("UNMUTE!");

            let member = match guild_id.member(&ctx, user_id) {
                Ok(member) => member,
                Err(e) => {
                    error!("Member not found: {:?}", e);
                    return;
                }
            };

            let name = member.display_name().to_string();

            let _ = announce(&ctx, channel_id, guild_id, &name, *user_id.as_u64());
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
fn my_help(
    context: &mut Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>
) -> CommandResult {
    help_commands::plain(context, msg, args, help_options, groups, owners)
}

fn main() {
    // Initialize the logger to use environment variables.
    //
    // In this case, a good default is setting the environment variable
    // `RUST_LOG` to debug`.
    env_logger::init();

    // Login with a bot token from the environment
    let token = env::var("DISCORD_APP_AUTH_TOKEN").expect("Expected a token in the environment");

    let mut client = Client::new(&token, Handler).expect("Error creating client");

    {
        let mut data = client.data.write();
        data.insert::<VoiceManager>(Arc::clone(&client.voice_manager));
    }

    client.with_framework(StandardFramework::new()
            .group(&GENERAL_GROUP)
            .help(&MY_HELP)
            .configure(|c| c
                .prefix("!")
                .allow_dm(false)
                .case_insensitivity(true)
                .allowed_channels(vec![ ChannelId(552168558323564544), 
                                        ChannelId(511144158975623169),
                                        ChannelId(739933045406171166)].into_iter().collect())
            )); // set the bot's prefix to "!"

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
            active_file     TEXT DEFAULT '',
            random          INTEGER DEFAULT 0,
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
    if let Err(why) = client.start() {
        error!("Client error: {:?}", why);
    }
}

fn announce(ctx: &Context, channel_id: ChannelId, guild_id: GuildId, name: &str, user_id: u64) {
    let db_path = Path::new("/config/database/db.sqlite");

    let db = match Connection::open(&db_path) {
        Ok(db) => db,
        Err(err) => {
            error!("Failed to open database: {}", err);
            return;
        }
    };

    let filename = match db.query_row::<String, _, _>(
        "SELECT active_file FROM names WHERE name=?1 AND user_id=?2",
        params![&name, user_id as i64],
        |row| row.get(0)).optional() {
            Ok(row) => row,
            Err(err) => {
                error!("Failed to query active file for {}, Error Code {}", name, err);
                return;
            }
    };

    let index_base_path = format!("/config/index/{}", &name);
    let files = read_dir(&index_base_path).ok();
    let path;
    if filename.is_some() && files.is_some() {
        let random = match db.query_row::<bool, _, _>(
            "SELECT random FROM names WHERE name=?1 AND user_id=?2",
            params![&name, user_id as i64],
            |row| row.get(0)) {
                Ok(row) => row,
                Err(err) => {
                    error!("Failed to query random file for {}, Error Code {}", name, err);
                    return;
                }
        };

        let count = files.unwrap().count();
        if random && count > 0 {
            let index = rand::thread_rng().gen_range(0, count);
            let mut paths = read_dir(&index_base_path).unwrap();
            path = paths.nth(index).unwrap().unwrap().path().to_str().unwrap().to_owned();
        } else {
            path = format!("{}/{}.wav", &index_base_path, &filename.unwrap());
        }
    } else {
        path = format!("/config/audio/{}.wav", &name);
    }

    check_path(&path, &name);

    play_file(ctx, channel_id, guild_id, &path);
}

fn play_file(ctx: &Context, channel_id: ChannelId, guild_id: GuildId, path: &str) {
    let manager_lock = &ctx
        .data
        .read()
        .get::<VoiceManager>()
        .cloned()
        .expect("Expected VoiceManager in ShareMap.");
    let mut manager = manager_lock.lock();

    if let Some(old_handler) = manager.get_mut(guild_id) {
        if let Some(old_channel_id) = old_handler.channel_id {
            if old_channel_id != channel_id {
                old_handler.stop();
            }
        }
    }

    let handler = match manager.join(guild_id, channel_id) {
        Some(handler) => handler,
        None => {
            error!("Joining voice channel");
            return;
        }
    };
    debug!("Joined {}", channel_id.mention());

    let source = match voice::ffmpeg(path) {
        Ok(source) => source,
        Err(err) => {
            error!("Err starting source: {:?}", err);
            return;
        }
    };

    info!("Playing sound file {}", path);
    handler.play(source);
}

fn voice_channel_is_empty(ctx: &Context, guild: &Guild, channel_id: ChannelId) -> bool {
    let mut is_empty = true;
    for state in guild
        .voice_states
        .values()
        .filter(|state| state.channel_id == Some(channel_id))
    {
        let user = match state.user_id.to_user(ctx) {
            Ok(user) => user,
            Err(err) => {
                error!("Error retrieving user: {:?}", err);
                return is_empty;
            }
        };
        is_empty &= user.bot;
    }
    return is_empty;
}

fn check_path(path: &str, name: &str) {
    if !Path::new(path).exists() {
        debug!("Didn't find file: {}.", path);
        debug!("Creating new file with espeak.");

        Command::new("espeak")
            .arg("-w")
            .arg(path)
            .arg(name)
            .output()
            .expect("Failed to run espeak!");
        let text_path = format!("/config/queue/{}", &name);

        fs::write(text_path, name).expect("Unable to write file");
    }
}

fn print_type_of<T>(_: &T) {
    println!("{}", std::any::type_name::<T>())
}
