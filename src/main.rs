#[macro_use] extern crate log;
#[macro_use] extern crate serenity;

extern crate env_logger;

use std::{
    env, 
    sync::Arc,
    path::Path,
    ptr::null_mut,
    ffi::CString,
    fs,
    fs::File,
    io::prelude::*,
    process::Command,
};

use serenity::client::bridge::voice::ClientVoiceManager;

use serenity::{client::{Context}, prelude::Mutex};

use serenity::{
    client::{
        CACHE,
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

        let user = match user_id.to_user() {
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

        let channel_id = match voice_state.channel_id {
                Some(channel_id) => channel_id,
                None => {
                    info!("Channel id not found.");
                    let manager_lock = _ctx.data.lock().get::<VoiceManager>().cloned().expect("Expected VoiceManager in ShareMap.");
                    let mut manager = manager_lock.lock();
                    manager.leave(guild_id);
                    return;
                }
            };

        let is_bot = user.bot;

        if !is_bot && !voice_state.self_mute {
            info!("UNMUTE!");

            let member = match guild_id.member(user_id) {
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

        // let channel = match channel_id.to_channel() {
        //     Ok(channel) => channel,
        //     Err(err) => {
        //         error!("Channel not found: {:?}", err);
        //         return;
        //     }
        // };

        // channel.mem

        // // TODO: disconnect when no human users in channel anymore
        // if 
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
        let mut data = client.data.lock();
        data.insert::<VoiceManager>(Arc::clone(&client.voice_manager));
    }

    client.with_framework(StandardFramework::new()
        .configure(|c| c.prefix("!")) // set the bot's prefix to "!"
        .cmd("newfile", newfile));

    // start listening for events by starting a single shard
    if let Err(why) = client.start() {
       error!("Client error: {:?}", why);
    }
}

fn announce(_ctx: Context, channel_id: ChannelId, guild_id: GuildId, name: String) {
    let manager_lock = _ctx.data.lock().get::<VoiceManager>().cloned().expect("Expected VoiceManager in ShareMap.");
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
        // if 
            // handler.join(channel_id);
            debug!("Joined {}", channel_id.mention());
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
        // } else {
        //     debug!("Not in a voice channel to play in");
        // }
    
}

fn check_path(path: &str, name: &str) {
    if !Path::new(path).exists() {
        debug!("Didn't find file: {}.", path);
        debug!("Creating new file with espeak.");

        Command::new("espeak").arg("-w").arg(path).arg(name).output().expect("Failed to run espeak!");
    }
}

command!(newfile(_context, message, args) {
    let channel_name = match message.channel_id.name() {
        Some(name) => name,
        None => {
            debug!("No channel name found");
            return Ok(());
        }
    };
    if channel_name == "announcer-bot-submissions" {

    let attachments = &message.attachments;
    if !attachments.is_empty() {
        let audio_file = &attachments[0];
        let content = match audio_file.download() {
            Ok(content) => content,
            Err(why) => {
                error!("Error downloading attachment: {:?}", why);
                // let _ = message.channel_id.say(, "Error downloading attachment");
                return Ok(());
            },
        };

        let mut name: &str = args.rest();

        let filename: String;
        if name.is_empty() {
            filename = audio_file.filename.to_owned();
        } else {
            filename = name.to_owned() + ".wav";
        }

        let mut file = match File::create("/config/audio/".to_owned() + &filename) {
            Ok(file) => file,
            Err(why) => {
                error!("Error creating file: {:?}", why);
                // let _ = message.channel_id.say("Error creating file");
                return Ok(());
            },
        };

        if let Err(why) = file.write(&content) {
            error!("Error writing to file: {:?}", why);
            return Ok(());
        }

        // normalise the audio file
        let normalise = Command::new("ffmpeg-normalize")
                .arg("-f")
                .arg("-c:a")
                .arg("libmp3lame")
                .arg("-b:a")
                .arg("128K")
                .arg(&filename)
                .arg("-o")
                .arg(&filename)
                .current_dir("/config/audio")
                .status()
                .expect("Failed to run ffmpeg-normalize");
        // println!("NORMALISE STATUS: {}", normalise);
        
        // trim length to 5s
        let trim = Command::new("ffmpeg")
                .arg("-i")
                .arg(&filename)
                .arg("-t")
                .arg("00:00:06")
                .arg(filename.to_owned() + "tmp.wav")
                .current_dir("/config/audio")
                .status()
                .expect("Failed to shorten length with ffmpeg");
        // println!("TRIM STATUS: {}", trim);
        
        
        fs::rename("/config/audio/".to_owned() + &filename + "tmp.wav", "/config/audio/".to_owned() + &filename)?;
        // println!("RENAME STATUS: {}", rename.);
    } else {

    }
    }
});
