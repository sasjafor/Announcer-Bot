#[macro_use]
extern crate log;

extern crate env_logger;
extern crate serenity;
extern crate url;

use url::{Url};
use serenity::prelude::TypeMapKey;
use serenity::prelude::Mutex;
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
            macros::{
                command,
                group,
            }, 
            Args,
            CommandResult,
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
    voice,
};

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

                if voice_channel_is_empty(&ctx, guild, self_channel_id) {
                    handler.leave();
                }
                return;
            }
        };

        const USER1_ID: UserId = UserId(239705630913331201);
        const USER2_ID: UserId = UserId(180995420196044809);

        if old_state.is_none() {
            if user_id == USER1_ID {
                let mut user_check = guild
                                    .voice_states
                                    .values()
                                    .filter(|state| state.channel_id == Some(channel_id))
                                    .filter(|state| state.user_id == USER2_ID)
                                    .peekable();
                    
                if user_check.peek().is_some() {
                    let path = "/config/StGallerConnection.mp3";
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
                    info!("cousins");
                    let path = "/config/StGallerConnection.mp3";
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

            announce(&ctx, channel_id, guild_id, &name);
            return;
        }
    }
}

#[group]
#[commands(newfile)]
struct General;

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
            .configure(|c| c
                .prefix("!")
            )); // set the bot's prefix to "!"

    let audio = Path::new("/config/audio");
    let queue = Path::new("/config/queue");
    let processing = Path::new("/config/processing/");

    if !audio.exists() {
        let _ = fs::create_dir(audio);
    }

    if !queue.exists() {
        let _ = fs::create_dir(queue);
    }

    if !processing.exists() {
        let _ = fs::create_dir(processing);
    }

    // start listening for events by starting a single shard
    if let Err(why) = client.start() {
        error!("Client error: {:?}", why);
    }
}

fn announce(ctx: &Context, channel_id: ChannelId, guild_id: GuildId, name: &str) {
    debug!("Joined {}", channel_id.mention());
    let path = format!("{}{}{}", "/config/audio/", &name, ".wav");

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
        let text_path = format!("{}{}", "/config/queue/", &name);

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

    if channel_name != "announcer-bot-submissions" {
        debug!("command used in wrong channel");
        return Ok(())
    }

    let arguments = args.raw_quoted().collect::<Vec<&str>>();

    let name = match arguments.first() {
        Some(name) => name,
        None => {
            let _ = message.channel_id.say(&ctx, "Please provide a name");
            return Ok(());
        }
    };
    let filename = format!("{}{}", &name, ".wav");
    let processing_path = "/config/processing/";
    let audio_path = "/config/audio/";

    if arguments.len() <= 2 {
        let attachments = &message.attachments;
        if !attachments.is_empty() {
            let audio_file = &attachments[0];
            let content = match audio_file.download() {
                Ok(content) => content,
                Err(why) => {
                    let _ = message.channel_id.say(&ctx, "Error downloading attachment");
                    error!("Error downloading attachment: {:?}", why);
                    return Ok(());
                }
            };

            let mut file = match File::create(format!("{}{}", processing_path, &filename)) {
                Ok(file) => file,
                Err(why) => {
                    let _ = message.channel_id.say(&ctx, "Error creating file");
                    error!("Error creating file: {:?}", why);
                    return Ok(());
                }
            };

            if let Err(why) = file.write(&content) {
                let _ = message.channel_id.say(&ctx, "Error writing file");
                error!("Error writing to file: {:?}", why);
                return Ok(());
            }
        } else {
            let _ = message.channel_id.say(&ctx, "Please attach an audio file");
            return Ok(())
        }
    } else {
        let url = arguments[1];
        let _ = match Url::parse(url) {
            Ok(url) => url,
            Err(why) => {
                let _ = message.channel_id.say(&ctx, "Please provide a valid url");
                debug!("Invalid url: {}", why);
                return Ok(());
            }
        };

        let start = arguments[2];
        let duration = arguments[3];

        let youtube_url = Command::new("youtube-dl")
            .arg("-g")
            .arg(url)
            .output()
            .expect("Failed to run youtube-dl");

        if !youtube_url.status.success() {
            let _ = message.channel_id.say(&ctx, "Youtube url error");
            error!("Error for youtube url {}", url);
            return Ok(())
        }

        let youtube_dloutput = match String::from_utf8(youtube_url.stdout) {
            Ok(res) => res,
            Err(why) => {
                let _ = message.channel_id.say(&ctx, "Failed to parse youtube-dl output");
                error!("Failed to parse youtube-dl output {}", why);
                return Ok(())
            }
        };
        let lines = youtube_dloutput.lines();

        let audio_url = match lines.last() {
            Some(line) => line,
            None => {
                let _ = message.channel_id.say(&ctx, "Youtube empty info");
                error!("Empty info for {}", url);
                return Ok(())
            }
        };

        let download_status = Command::new("ffmpeg")
            .arg("-y")
            .arg("-ss")
            .arg(start.to_string())
            .arg("-i")
            .arg(audio_url)
            .arg("-t")
            .arg(duration.to_string())
            .arg("-vn")
            .arg("-f")
            .arg("wav")
            .arg(&filename)
            .current_dir(&processing_path)
            .output()
            .expect("failed to run ffmpeg")
            .status;
        
        if !download_status.success() {
            let _ = message.channel_id.say(&ctx, "Failed to download from youtube");
            error!("Failed to run ffmpeg to download audio for file {}; CODE: {}", &filename, download_status.code().expect("no exit code"));
            return Ok(());
        }
    }

    let filter_filename = format!("{}{}", &name, ".filter.wav");
    let normalise_filename = format!("{}{}", &name, ".normalise.wav");
    let trim_filename = format!("{}{}", &name, ".trim.wav");

    if arguments.len() == 2 || arguments.len() == 5 {
        let filter_string = match arguments.last() {
            Some(filter) => filter,
            None =>  {
                error!("There was no argument when there should be!");
                return Ok(());
            }
        };

        let filter_status = Command::new("ffmpeg")
            .arg("-y")
            .arg("-i")
            .arg(&filename)
            .arg("-filter:a")
            .arg(&filter_string)
            .arg("-f")
            .arg("wav")
            .arg(&filter_filename)
            .current_dir(&processing_path)
            .output()
            .expect("Failed to run ffmpeg")
            .status;
        
        if !filter_status.success() {
            let _ = message.channel_id.say(&ctx, "Failed to apply audio filter");
            error!("Failed to apply audio effect for file {}; CODE: {}", &filename, filter_status.code().expect("no exit code"));
            return Ok(());
        }

    } else {
        let _ = match fs::copy(
            format!("{}{}", &processing_path, &filename),
            format!("{}{}", &audio_path, &filter_filename),
        ) {
            Ok(res) => res,
            Err(why) => {
                let _ = message.channel_id.say(&ctx, "Failed to copy file");
                error!("Failed to copy file {} ERROR: {}", &filename, why);
                return Ok(());
            }
        };
    }

    // normalise the audio file
    let normalise_status = Command::new("ffmpeg-normalize")
        .arg("-f")
        .arg("-c:a")
        .arg("libmp3lame")
        .arg("-b:a")
        .arg("128K")
        .arg(&filter_filename)
        .arg("-o")
        .arg(&normalise_filename)
        .current_dir(&processing_path)
        .output()
        .expect("Failed to run ffmpeg-normalize")
        .status;

    if !normalise_status.success()
    {
        let _ = message.channel_id.say(&ctx, "Failed to normalise audio");
        error!("Failed to run ffmpeg-normalize for file {} CODE: {}", &filename, normalise_status.code().expect("no exit code"));
        return Ok(());
    }

    // trim length to 5s
    let trim_status = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(&normalise_filename)
        .arg("-t")
        .arg("00:00:06")
        .arg("-f")
        .arg("wav")
        .arg(&trim_filename)
        .current_dir(&processing_path)
        .output() 
        .expect("Failed to run fmmpeg")
        .status;

    if !trim_status.success()
    {
        let _ = message.channel_id.say(&ctx, "Failed to shorten length with ffmpeg");
        error!("Failed to shorten length with ffmpeg for file {} ERROR: {}", &filename, trim_status.code().expect("no exit code"));
        return Ok(());
    };

    let _ = match fs::rename(
        format!("{}{}", &processing_path, &trim_filename),
        format!("{}{}", &audio_path, &filename),
    ) {
        Ok(res) => res,
        Err(why) => {
            let _ = message.channel_id.say(&ctx, "Failed to rename file");
            error!("Failed to rename file {} ERROR: {}", &trim_filename, why);
            return Ok(());
        }
    };

    let text_path = format!("{}{}", "/config/queue/", &name);

    let _ = match fs::remove_file(&text_path) {
        Ok(res) => res,
        Err(why) => {
            debug!("Failed to remove queue file {} ERROR: {}", &text_path, why);
        }
    };

    let _ = match fs::remove_file(format!("{}{}", &processing_path, &filename)) {
        Ok(res) => res,
        Err(why) => {
            debug!("Failed to remove queue file {} ERROR: {}", &filename, why);
        }
    };

    let _ = match fs::remove_file(format!("{}{}", &processing_path, &filter_filename)) {
        Ok(res) => res,
        Err(why) => {
            debug!("Failed to remove queue file {} ERROR: {}", &filter_filename, why);
        }
    };

    let _ = match fs::remove_file(format!("{}{}", &processing_path, &normalise_filename)) {
        Ok(res) => res,
        Err(why) => {
            debug!("Failed to remove queue file {} ERROR: {}", &normalise_filename, why);
        }
    };

    let _ = message.channel_id.say(&ctx, format!("Successfully added new file for {}", name));
    Ok(())
}
