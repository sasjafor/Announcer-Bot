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
#[description("Submit a new announcement either as file or url")]
#[usage("<discordname> <announcement-name> [<filters>]\nnewfile <discordname> <announcement-name> <url> <start time> <duration> [<filters>]")]
#[example("\"Mr Yzarul\" \"funny noise\"")]
#[example("\"Mr Yzarul\" \"funny noise\" \"vibrato\"")]
#[example("\"Mr Yzarul\" \"funny noise\" \"https://www.youtube.com/watch?v=dQw4w9WgXcQ\" 20 7")]
#[example("\"Mr Yzarul\" \"funny noise\" \"https://www.youtube.com/watch?v=dQw4w9WgXcQ\" 02:20 4")]
#[example("See all filters here https://ffmpeg.org/ffmpeg-filters.html")]
#[min_args(2)]
#[max_args(6)]
#[help_available]
pub fn newfile(ctx: &mut Context, message: &Message, args: Args) -> CommandResult {
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
            .arg(format!("{}{}", "file:", &filename))
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

    let processed_filename = format!("{}{}", &name, ".processed.wav");

    let mut filter_string = "";

    if arguments.len() == 3 || arguments.len() == 6 {
        filter_string = match arguments.last() {
            Some(filter) => filter,
            None =>  {
                error!("There was no argument when there should be!");
                return Ok(());
            }
        };
    }

    let normalize_and_filter_string;
    if filter_string.len() > 0 {
        normalize_and_filter_string = format!("{}{}", "loudnorm,", &filter_string);
    } else {
        normalize_and_filter_string = "loudnorm".to_string();
    }

    let filter_output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-t")
        .arg("00:00:06")
        .arg("-i")
        .arg(format!("{}{}", "file:", &filename))
        .arg("-filter:a")
        .arg(&normalize_and_filter_string)
        .arg("-f")
        .arg("wav")
        .arg(format!("{}{}", "file:", &processed_filename))
        .current_dir(&processing_path)
        .output()
        .expect("Failed to run ffmpeg");
    
    if !filter_output.status.success() {
        let _ = message.channel_id.say(&ctx, "Failed to apply audio filter");
        let _ = delete_processing_files(&processing_path, &filename, &processed_filename);
        error!("Failed to apply audio effect for file {}; CODE: {}", &filename, filter_output.status.code().expect("no exit code"));
        return Ok(());
    }

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

    let user_id = message.author.id.as_u64();
    let _ = match db.execute(
        "INSERT OR REPLACE INTO names (name, user_id, active_file)
            VALUES (?1, ?2, ?3)",
        params![&name, *user_id as i64, &index_name]) {
            Ok(_) => (),
            Err(err) => {
                error!("Failed to insert new name, Error Code {}", err);
                return Ok(());
            }
    };

    let _ = match fs::rename(
        format!("{}{}", &processing_path, &processed_filename),
        format!("{}{}{}{}{}", &indexed_path, &name, "/", &index_name, ".wav"),
    ) {
        Ok(res) => res,
        Err(why) => {
            let _ = message.channel_id.say(&ctx, "Failed to rename file");
            let _ = delete_processing_files(&processing_path, &filename, &processed_filename);
            error!("Failed to rename file {} ERROR: {}", &processed_filename, why);
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

    let _ = delete_processing_files(&processing_path, &filename, &processed_filename);

    let _ = message.channel_id.say(&ctx, format!("Successfully added new file for {}", name));
    Ok(())
}

fn delete_processing_files(processing_path: &str, filename: &str, processed_filename: &str) {
    let _ = match fs::remove_file(format!("{}{}", &processing_path, &filename)) {
        Ok(res) => res,
        Err(why) => {
            debug!("Failed to remove queue file {} ERROR: {}", &filename, why);
        }
    };

    let _ = match fs::remove_file(format!("{}{}", &processing_path, &processed_filename)) {
        Ok(res) => res,
        Err(why) => {
            debug!("Failed to remove queue file {} ERROR: {}", &processed_filename, why);
        }
    };
}