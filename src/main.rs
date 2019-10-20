#[macro_use]
extern crate log;

extern crate env_logger;
extern crate serenity;

use std::{
    env, 
    fs, 
    fs::File, 
    io::prelude::*, 
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
            macros::command, 
            Args,
            CommandResult,
        },
        StandardFramework
    },
    model::{
        event::ResumedEvent, 
        gateway::Ready, 
        guild::Guild, 
        id::ChannelId, 
        id::GuildId,
        prelude::Message,
        voice::VoiceState,
    },
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

    fn voice_state_update(&self, ctx: Context, guild_id: Option<GuildId>, _old: Option<VoiceState>, new_state: VoiceState) {
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

        let channel_id = match new_state.channel_id {
            Some(channel_id) => channel_id,
            None => {
                info!("Channel id not found.");
                let data = &ctx.data.read();
                let manager_lock = data
                    .get::<VoiceManager>()
                    .cloned()
                    .expect("Expected VoiceManager in ShareMap.");

                let mut manager = manager_lock.lock();
                let handler = match manager.get_mut(guild_id) {
                    Some(handler) => handler,
                    None => {
                        info!("No handler found.");
                        return;
                    }
                };
                let self_channel_id = match handler.channel_id {
                    Some(id) => id,
                    None => {
                        info!("Not connected to a channel.");
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

                if voice_channel_is_empty(&ctx, guild, self_channel_id) {
                    handler.leave();
                }
                return;
            }
        };

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

            announce(ctx, channel_id, guild_id, name);
            return;
        }
    }
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

    client.with_framework(StandardFramework::new().configure(|c| c.prefix("!"))); // set the bot's prefix to "!"

    let audio = Path::new("/config/audio");
    let queue = Path::new("/config/queue");

    if !audio.exists() {
        let _ = fs::create_dir(audio);
    }

    if !queue.exists() {
        let _ = fs::create_dir(queue);
    }

    // start listening for events by starting a single shard
    if let Err(why) = client.start() {
        error!("Client error: {:?}", why);
    }
}

fn announce(ctx: Context, channel_id: ChannelId, guild_id: GuildId, name: String) {
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
    let path = "/config/audio/".to_owned() + &name + ".wav";

    info!("Path={}", path);

    check_path(&path, &name);

    let source = match voice::ffmpeg(path) {
        Ok(source) => source,
        Err(err) => {
            error!("Err starting source: {:?}", err);
            return;
        }
    };
    handler.play(source);

    info!("Playing sound file for {}", name);
}

fn voice_channel_is_empty(ctx: &Context, guild: Guild, channel_id: ChannelId) -> bool {
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
        let text_path = "/config/queue/".to_owned() + &name;

        fs::write(text_path, name).expect("Unable to write file");
    }
}

#[command]
pub fn newfile(ctx: &mut Context, message: &Message, args: Args) -> CommandResult {
    let channel_name = match message.channel_id.name(&ctx) {
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
                    let _ = message.channel_id.say(&ctx, "Error downloading attachment");
                    return Ok(());
                }
            };

            let name: &str = args.rest();

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
                    let _ = message.channel_id.say(ctx, "Error creating file");
                    return Ok(());
                }
            };

            if let Err(why) = file.write(&content) {
                error!("Error writing to file: {:?}", why);
                return Ok(());
            }

            // normalise the audio file
            let _normalise = Command::new("ffmpeg-normalize")
                .arg("-f")
                .arg("-c:a")
                .arg("libmp3lame")
                .arg("-b:a")
                .arg("128K")
                .arg(&filename)
                .arg("-o")
                .arg(&filename)
                .current_dir("/config/audio")
                .output()
                .expect("Failed to run ffmpeg-normalize");

            // trim length to 5s
            let _trim = Command::new("ffmpeg")
                .arg("-i")
                .arg(&filename)
                .arg("-t")
                .arg("00:00:06")
                .arg(filename.to_owned() + "tmp.wav")
                .current_dir("/config/audio")
                .output()
                .expect("Failed to shorten length with ffmpeg");

            fs::rename(
                "/config/audio/".to_owned() + &filename + "tmp.wav",
                "/config/audio/".to_owned() + &filename,
            )?;
        } else {
            let _ = message.channel_id.say(&ctx, "Please attach an audio file");
        }
    }

    Ok(())
}
