use std::{fs, fs::File, io::prelude::*, path::Path, process::Command, time::Duration};

use serenity::{
    builder::{CreateApplicationCommand, CreateComponents, CreateEmbed},
    client::Context,
    model::{interactions::application_command::ApplicationCommandOptionType, prelude::*},
};

use rusqlite::{params, Connection};
use tracing::{debug, error};
use url::Url;

use crate::lib::parse::parse_duration;

pub fn create_new_command(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    return command
        .name("new")
        .description("Submit a new announcement either as file or url")
        .create_option(|option| {
            option
                .name("file")
                .description("Add new announcement using a file.")
                .kind(ApplicationCommandOptionType::SubCommand)
                .create_sub_option(|option| {
                    option
                        .name("user")
                        .description("The user for which to add an announcement.")
                        .kind(ApplicationCommandOptionType::User)
                        .required(true)
                })
                .create_sub_option(|option| {
                    option
                        .name("announcement")
                        .description("Name of the announcement.")
                        .kind(ApplicationCommandOptionType::String)
                        .required(true)
                })
                .create_sub_option(|option| {
                    option
                        .name("file")
                        .description("Audio file to be used as announcement.")
                        .kind(ApplicationCommandOptionType::Attachment)
                        .required(true)
                })
                .create_sub_option(|option| {
                    option
                        .name("filters")
                        .description("FFMPEG filters to transform audio.")
                        .kind(ApplicationCommandOptionType::String)
                })
        })
        .create_option(|option| {
            option
                .name("url")
                .description("Add new announcement using a url.")
                .kind(ApplicationCommandOptionType::SubCommand)
                .create_sub_option(|option| {
                    option
                        .name("user")
                        .description("The user for which to add an announcement.")
                        .kind(ApplicationCommandOptionType::User)
                        .required(true)
                })
                .create_sub_option(|option| {
                    option
                        .name("announcement")
                        .description("Name of the announcement.")
                        .kind(ApplicationCommandOptionType::String)
                        .required(true)
                })
                .create_sub_option(|option| {
                    option
                        .name("url")
                        .description("URL for the announcement.")
                        .kind(ApplicationCommandOptionType::String)
                        .required(true)
                })
                .create_sub_option(|option| {
                    option
                        .name("start")
                        .description("Start time.")
                        .kind(ApplicationCommandOptionType::String)
                        .required(true)
                })
                .create_sub_option(|option| {
                    option
                        .name("end")
                        .description("End time.")
                        .kind(ApplicationCommandOptionType::String)
                        .required(true)
                })
                .create_sub_option(|option| {
                    option
                        .name("filters")
                        .description("FFMPEG filters to transform audio.")
                        .kind(ApplicationCommandOptionType::String)
                })
        });
}

// #[command]
// #[description("Submit a new announcement either as file or url")]
// #[usage("<discordname> <announcement-name> [<filters>]\nnewfile <discordname> <announcement-name> <url> <start time> <duration> [<filters>]")]
// #[example("\"Mr Yzarul\" \"funny noise\"")]
// #[example("\"Mr Yzarul\" \"funny noise\" \"vibrato\"")]
// #[example("\"Mr Yzarul\" \"funny noise\" \"https://www.youtube.com/watch?v=dQw4w9WgXcQ\" 20 7")]
// #[example("\"Mr Yzarul\" \"funny noise\" \"https://www.youtube.com/watch?v=dQw4w9WgXcQ\" 02:20 4")]
// #[example("See all filters here https://ffmpeg.org/ffmpeg-filters.html")]
// #[min_args(2)]
// #[max_args(6)]
// #[help_available]
pub async fn new_file(
    ctx: &Context,
    name: &String,
    announcement_name: &String,
    audio_file: &Attachment,
    user: &User,
    filters: Option<&String>,
) -> (String, Option<CreateEmbed>, Option<CreateComponents>) {
    let filename = format!("{}.wav", &name);
    let processing_path = "/config/processing/";

    let content = match audio_file.download().await {
        Ok(content) => content,
        Err(why) => {
            let err_str = "Error downloading attachment".to_string();
            error!("{}: {}", err_str, why);
            return (err_str, None, None);
        }
    };

    let mut file = match File::create(format!("{}{}", processing_path, &filename)) {
        Ok(file) => file,
        Err(why) => {
            let err_str = "Error creating file".to_string();
            error!("{}: {}", err_str, why);
            return (err_str, None, None);
        }
    };

    if let Err(why) = file.write(&content) {
        let err_str = "Error writing file".to_string();
        error!("{}: {}", err_str, why);
        return (err_str, None, None);
    }

    return new(ctx, name, announcement_name, user, filters).await;
}

pub async fn new_url(
    ctx: &Context,
    name: &String,
    announcement_name: &String,
    url: &String,
    start: &String,
    end: &String,
    user: &User,
    filters: Option<&String>,
) -> (String, Option<CreateEmbed>, Option<CreateComponents>) {
    let filename = format!("{}.wav", &name);
    let processing_path = "/config/processing/";

    let _ = match Url::parse(url) {
        Ok(url) => url,
        Err(why) => {
            let err_str = "Please provide a valid url".to_string();
            error!("{}: {}", err_str, why);
            return (err_str, None, None);
        }
    };

    let start_parsed = parse_duration(start).unwrap();
    let end_parsed = parse_duration(end).unwrap();
    let duration = end_parsed - start_parsed;

    if duration > Duration::from_secs(7) {
        let err_str = "Duration is too long".to_string();
        debug!("{}: {}", err_str, duration.as_secs_f64());
        return (err_str, None, None);
    }

    let youtube_url = Command::new("youtube-dl")
        .arg("-g")
        .arg(url)
        .output()
        .expect("Failed to run youtube-dl");

    if !youtube_url.status.success() {
        let err_str = "Youtube-dl Error: It likely needs an update".to_string();
        error!("{}: url = {} err: {}", err_str, url, youtube_url.status);
        return (err_str, None, None);
    }

    let youtube_dloutput = match String::from_utf8(youtube_url.stdout) {
        Ok(res) => res,
        Err(why) => {
            let err_str = "Failed to parse youtube-dl output".to_string();
            error!("{}: {}", err_str, why);
            return (err_str, None, None);
        }
    };
    let lines = youtube_dloutput.lines();

    let audio_url = match lines.last() {
        Some(line) => line,
        None => {
            let err_str = "Youtube empty info".to_string();
            error!("{}: {}", err_str, url);
            return (err_str, None, None);
        }
    };

    let download_status = Command::new("ffmpeg")
        .arg("-y")
        .arg("-ss")
        .arg(start)
        .arg("-to")
        .arg(end)
        .arg("-i")
        .arg(audio_url)
        .arg("-vn")
        .arg("-f")
        .arg("wav")
        .arg(format!("file:{}", &filename))
        .current_dir(&processing_path)
        .output()
        .expect("failed to run ffmpeg")
        .status;

    if !download_status.success() {
        let err_str = "Failed to run ffmpeg to download audio".to_string();
        error!(
            "{} for file {}, error_code = {}",
            err_str,
            &filename,
            download_status.code().expect("no exit code")
        );
        return (err_str, None, None);
    }

    return new(ctx, name, announcement_name, user, filters).await;
}

pub async fn new(
    _ctx: &Context,
    name: &String,
    announcement_name: &String,
    user: &User,
    filters: Option<&String>,
) -> (String, Option<CreateEmbed>, Option<CreateComponents>) {
    let filename = format!("{}.wav", &name);
    let processed_filename = format!("{}{}", &name, ".processed.wav");
    let processing_path = "/config/processing/";
    let indexed_path = "/config/index/";
    let db_path = Path::new("/config/database/db.sqlite");

    let normalize_and_filter_string;
    if filters.is_some() {
        normalize_and_filter_string = format!("{},loudnorm", filters.unwrap());
    } else {
        normalize_and_filter_string = "loudnorm".to_string();
    }

    let filter_output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-t")
        .arg("00:00:06")
        .arg("-i")
        .arg(format!("file:{}", &filename))
        .arg("-filter:a")
        .arg(&normalize_and_filter_string)
        .arg("-ar")
        .arg("48000")
        .arg("-f")
        .arg("wav")
        .arg(format!("file:{}", &processed_filename))
        .current_dir(&processing_path)
        .output()
        .expect("Failed to run ffmpeg");

    debug!("ffmpeg -y -t 00:00:06 -i {} -filter:a {} -ar 48000 -f wav {}", format!("file:{}", &filename), &normalize_and_filter_string, format!("file:{}", &processed_filename));

    if !filter_output.status.success() {
        // let _ = delete_processing_files(&processing_path, &filename, &processed_filename);

        let err_str = "Failed to apply audio filter".to_string();
        error!(
            "{} for file {}, error_code = {}",
            err_str,
            &filename,
            filter_output.status.code().expect("no exit code")
        );
        return (err_str, None, None);
    }

    let name_path = format!("{}{}", &indexed_path, &name);

    if !Path::new(&name_path).exists() {
        let _ = fs::create_dir(name_path).expect("Failed to create directory.");
    }

    let db = match Connection::open(&db_path) {
        Ok(db) => db,
        Err(why) => {
            let err_str = "Failed to open database".to_string();
            error!("{}: {}", err_str, why);
            return (err_str, None, None);
        }
    };

    let user_id = user.id.as_u64();
    let _ = match db.execute(
        "INSERT OR REPLACE INTO names (name, user_id, active_file)
            VALUES (?1, ?2, ?3)",
        params![&name, *user_id as i64, announcement_name],
    ) {
        Ok(_) => (),
        Err(why) => {
            let err_str = "Failed to insert new name".to_string();
            error!("{}: {}", err_str, why);
            return (err_str, None, None);
        }
    };

    let _ = match fs::rename(
        format!("{}{}", &processing_path, &processed_filename),
        format!("{}{}/{}.wav", &indexed_path, &name, announcement_name),
    ) {
        Ok(res) => res,
        Err(why) => {
            let _ = delete_processing_files(&processing_path, &filename, &processed_filename);
            let err_str = "Failed to rename file".to_string();
            error!("{} for file {}: {}", err_str, &processed_filename, why);
            return (err_str, None, None);
        }
    };

    let text_path = format!("/config/queue/{}", &name);

    let _ = match fs::remove_file(&text_path) {
        Ok(res) => res,
        Err(why) => {
            debug!("Failed to remove queue file {} ERROR: {}", &text_path, why);
        }
    };

    let _ = delete_processing_files(&processing_path, &filename, &processed_filename);

    return (format!("Successfully added new file for {}", name), None, None);
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
