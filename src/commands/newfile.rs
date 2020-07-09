use std::{
    fs, 
    fs::File, 
    io::prelude::*,
    path::Path, 
    process::Command, 
    time::Duration,
};

use serenity::{
    client::{
        Context,
    },
    framework::{
        standard::{
            macros::{
                command,
            }, 
            Args,
            CommandResult,
        },
    },
    model::{
        prelude::*,
    },
};

use rusqlite::{
    params,
    Connection,
};

use url::{Url};

use lib::msg::check_msg;
use lib::parse::parse_duration;

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
            check_msg(message.channel_id.say(&ctx, "Please provide a name"));
            return Ok(());
        }
    };

    if arguments.len() < 2 || (arguments.len() > 3 && arguments.len() < 5) {
        check_msg(message.channel_id.say(&ctx, "Please provide a name for this announcement"));
        return Ok(());
    }

    let index_name = arguments[1];
    let filename = format!("{}{}", &name, ".wav");
    let processing_path = "/config/processing/";
    let indexed_path = "/config/index/";
    let db_path = Path::new("/config/database/db.sqlite");

    

    if arguments.len() <= 3 {
        let attachments = &message.attachments;
        if !attachments.is_empty() {
            let audio_file = &attachments[0];
            let content = match audio_file.download() {
                Ok(content) => content,
                Err(why) => {
                    check_msg(message.channel_id.say(&ctx, "Error downloading attachment"));
                    error!("Error downloading attachment: {:?}", why);
                    return Ok(());
                }
            };

            let mut file = match File::create(format!("{}{}", processing_path, &filename)) {
                Ok(file) => file,
                Err(why) => {
                    check_msg(message.channel_id.say(&ctx, "Error creating file"));
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
        let url = arguments[2];
        let _ = match Url::parse(url) {
            Ok(url) => url,
            Err(why) => {
                let _ = message.channel_id.say(&ctx, "Please provide a valid url");
                debug!("Invalid url: {}", why);
                return Ok(());
            }
        };

        let start = arguments[3];
        let duration = arguments[4];

        let duration_parsed = parse_duration(duration).unwrap();

        if duration_parsed > Duration::from_secs(7) {
            let _ = message.channel_id.say(&ctx, "Duration too long");
            debug!("Duration is too long {}", duration);
            return Ok(())
        }

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
            .arg("-t")
            .arg(duration.to_string())
            .arg("-i")
            .arg(audio_url)
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

    if arguments.len() == 3 || arguments.len() == 6 {
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
            let _ = delete_processing_files(&processing_path, &filename, &filter_filename, &normalise_filename);
            error!("Failed to apply audio effect for file {}; CODE: {}", &filename, filter_status.code().expect("no exit code"));
            return Ok(());
        }

    } else {
        let _ = match fs::copy(
            format!("{}{}", &processing_path, &filename),
            format!("{}{}", &processing_path, &filter_filename),
        ) {
            Ok(res) => res,
            Err(why) => {
                let _ = message.channel_id.say(&ctx, "Failed to copy file");
                let _ = delete_processing_files(&processing_path, &filename, &filter_filename, &normalise_filename);
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
        let _ = delete_processing_files(&processing_path, &filename, &filter_filename, &normalise_filename);
        error!("Failed to run ffmpeg-normalize for file {} CODE: {}", &filename, normalise_status.code().expect("no exit code"));
        return Ok(());
    }

    // trim length to 5s
    let trim_status = Command::new("ffmpeg")
        .arg("-y")
        .arg("-t")
        .arg("00:00:06")
        .arg("-i")
        .arg(&normalise_filename)
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
        let _ = delete_processing_files(&processing_path, &filename, &filter_filename, &normalise_filename);
        error!("Failed to shorten length with ffmpeg for file {} ERROR: {}", &filename, trim_status.code().expect("no exit code"));
        return Ok(());
    };

    let name_path = format!("{}{}", &indexed_path, &name);

    if !Path::new(&name_path).exists() {
        let _ = fs::create_dir(name_path)?;
    }

    let db = match Connection::open(&db_path) {
        Ok(db) => db,
        Err(err) => {
            error!("Failed to open database: {}", err);
            return Ok(());
        }
    };

    let _ = match db.execute(
        "INSERT OR REPLACE INTO names (name, active_file)
            VALUES (?1, ?2)",
        params![&name, &index_name]) {
            Ok(_) => (),
            Err(err) => {
                error!("Failed to insert new name, Error Code {}", err);
                return Ok(());
            }
    };

    let _ = match fs::rename(
        format!("{}{}", &processing_path, &trim_filename),
        format!("{}{}{}{}{}", &indexed_path, &name, "/", &index_name, ".wav"),
    ) {
        Ok(res) => res,
        Err(why) => {
            let _ = message.channel_id.say(&ctx, "Failed to rename file");
            let _ = delete_processing_files(&processing_path, &filename, &filter_filename, &normalise_filename);
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

    let _ = delete_processing_files(&processing_path, &filename, &filter_filename, &normalise_filename);

    let _ = message.channel_id.say(&ctx, format!("Successfully added new file for {}", name));
    Ok(())
}

fn delete_processing_files(processing_path: &str, filename: &str, filter_filename: &str, normalise_filename: &str) {
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
}