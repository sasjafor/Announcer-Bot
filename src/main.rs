#[macro_use] extern crate log;
#[macro_use] extern crate serenity;

extern crate env_logger;
extern crate espeak_sys;
extern crate libc;

use std::{
    env, 
    sync::Arc,
    path::Path,
    ptr::null_mut,
    ffi::CString,
    fs::File,
    io::prelude::*,
};

use espeak_sys::{
    espeakCHARS_AUTO,
    espeak_POSITION_TYPE,
    espeak_Synth,
};

use libc::c_void;

use serenity::client::bridge::voice::ClientVoiceManager;

use serenity::{client::{Context}, prelude::Mutex};

use serenity::{
    client::{
        Cache,
        Client,
        EventHandler,
    },
    framework::StandardFramework,
    model::{
        event::ResumedEvent,
        gateway::Ready,
        id::GuildId,
        id::ChannelId,
        voice::VoiceState,
        channel::Message,
    },
    Result as SerenityResult,
    http,
    voice,
};

use serenity::prelude::*;

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

    fn voice_state_update(&self, _ctx: Context, guild_id: Option<GuildId>, voice_state: VoiceState) {
        let user_id = voice_state.user_id;

        let user = match user_id.to_user(&_ctx) {
            Ok(user) => user,
            Err(e) => {
                error!("User not found: {:?}", e);
                return;
            }
        };

        let is_bot = user.bot;

        if !is_bot && !voice_state.self_mute {
            info!("UNMUTE!");

            let channel_id = match voice_state.channel_id {
                Some(channel_id) => channel_id,
                None => {
                    info!("Channel id not found.");
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

            let member = match guild_id.member(&_ctx, user_id) {
                Ok(member) => member,
                Err(e) => {
                    error!("Member not found: {:?}", e);
                    return;
                }
            };

            let name = member.display_name().to_string();

            announce(_ctx, channel_id, guild_id, name);
            return;
        }

        // TODO: disconnect when no human users in channel anymore
    }
}

fn main() {
    // Initialize the logger to use environment variables.
    //
    // In this case, a good default is setting the environment variable
    // `RUST_LOG` to debug`.
    env_logger::init();

    // Login with a bot token from the environment
    let token = env::var("DISCORD_APP_AUTH_TOKEN")
        .expect("Expected a token in the environment");

    let mut client = Client::new(&token, Handler)
        .expect("Error creating client");

    {
        let mut data = client.data.write();
        data.insert::<VoiceManager>(Arc::clone(&client.voice_manager));
    }

    client.with_framework(StandardFramework::new()
        .configure(|c| c.prefix("!")) // set the bot's prefix to "!"
        .cmd("ping", ping));

    // start listening for events by starting a single shard
    if let Err(why) = client.start() {
       error!("Client error: {:?}", why);
    }
}

fn announce(_ctx: Context, channel_id: ChannelId, guild_id: GuildId, name: String) {
    let manager_lock = _ctx.data.write().get::<VoiceManager>().cloned().expect("Expected VoiceManager in ShareMap.");
    let mut manager = manager_lock.lock();

    if let Some(old_handler) = manager.get_mut(guild_id) {
        if let Some(old_channel_id) = old_handler.channel_id {
            if old_channel_id != channel_id {
                old_handler.stop();
            }
        }
    }

    if manager.join(guild_id, channel_id).is_some() {
        debug!("Joined {}", channel_id.mention());
        if let Some(handler) = manager.get_mut(guild_id) {
            let path = "/config/audio/".to_owned() + &name + ".wav";

            info!("Path={}", path);

            check_path(&path, &name);

            let source = match voice::ffmpeg(path) {
                Ok(source) => source,
                Err(why) => {
                    error!("Err starting source: {:?}", why);
                    return;
                },
            };
            handler.play(source);
            info!("Playing sound file for {}", name);
        } else {
            debug!("Not in a voice channel to play in");
        }
    } else {
        error!("Error joining the channel");
    }
}

fn check_path(path: &str, name: &str) {
    if !Path::new(path).exists() {
        debug!("Didn't find file: {}.", path);
        debug!("Creating new file with espeak.");

        let mut data = Vec::new();
        let bufptr = data.as_mut_ptr() as *mut c_void;
        let c_str = CString::new(name).unwrap();
        let c_null = null_mut();
        unsafe {
            espeak_Synth(c_str.as_ptr() as *const c_void, name.len() as u64, 0, espeak_POSITION_TYPE::POS_CHARACTER, 
                0, espeakCHARS_AUTO, c_null, bufptr);
        }

        let mut file = match File::create(path) {
            Ok(file) => file,
            Err(err) => {
                error!("Error creating file {}", err);
                return;
            } 
        };

        let user_data = &data[..];

        match file.write_all(user_data) {
            Ok(()) => return,
            Err(err) => {
                error!("Error writing to file {}", err);
            }
        };
    }
}

command!(ping(_context, message) {
    info!("SENDING PONG!");
    let _ = message.reply(_context, "Pong!");
});
