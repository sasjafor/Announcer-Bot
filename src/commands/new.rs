use poise::CreateReply;
use rusqlite::{params, Connection};
use std::{fs::{self, File}, io::prelude::*, path::Path, process::Command, time::Duration};
use tracing::debug;
use url::Url;

use serenity::{all::CreateEmbed, model::{
        colour::Colour,
        prelude::*,
    }};

use crate::{
    util::{
        consts::{BOT_ADMIN_USER_ID, ELEMENT_LABEL_LENGTH}, 
        parse::parse_duration, 
        util::{send_debug, send_error}
    },
    PContext, PError,
};

fn new_help() -> String {
    return "\
Submit a new announcement either as file or url.
Usage:  
!new file <discordname> <announcement-name> [<filters>]
!new url <discordname> <announcement-name> <url> <start time> <end time> [<filters>]

Examples:
!new file @Yzarul \"funny noise\"
!new file @Yzarul \"funny noise\" \"vibrato\"
!new url @Yzarul \"funny noise\" \"https://www.youtube.com/watch?v=dQw4w9WgXcQ\" 20 25
!new url @Yzarul \"funny noise\" \"https://www.youtube.com/watch?v=dQw4w9WgXcQ\" 02:20 02:25
!new url @Yzarul \"funny noise\" \"https://www.youtube.com/watch?v=dQw4w9WgXcQ\" 02:20 02:25 vibrato=d=1.0

See all filters here https://ffmpeg.org/ffmpeg-filters.html
"
    .to_string();
}

#[doc = "Submit a new announcement either as file or url."]
#[poise::command(
    category = "Main Commands",
    guild_only,
    prefix_command,
    slash_command,
    required_bot_permissions = "SEND_MESSAGES",
    subcommands("file", "url"),
    help_text_fn = "new_help"
)]
pub async fn new(_ctx: PContext<'_>) -> Result<(), PError> {
    return Ok(());
}

#[doc = "Add new announcement using a file."]
#[poise::command(
    category = "Main Commands",
    guild_only,
    prefix_command,
    slash_command,
    required_bot_permissions = "SEND_MESSAGES"
)]
pub async fn file(
    ctx: PContext<'_>,
    #[description = "The user for which to add an announcement."] user: User,
    #[description = "Name of the announcement."] announcement: String,
    #[description = "Audio file to be used as announcement."] file: Attachment,
    #[description = "FFMPEG filters to transform audio."] filters: Option<String>,
    #[description = "Override the length limit"] override_length_limit: Option<bool>,
) -> Result<(), PError> {
    let mut override_length = false;
    if override_length_limit.is_some() {
        override_length = override_length_limit.unwrap();
        if override_length {
            if ctx.author().id.get() != BOT_ADMIN_USER_ID {
                let why = "".to_string();
                let err_str = "You are not allowed to use the length override!".to_string();
                return send_debug(ctx, err_str, why).await;
            }
        }
    }

    let announcement_name_length = announcement.chars().count();
    if announcement_name_length > ELEMENT_LABEL_LENGTH {
        let why = announcement_name_length;
        let err_str = "Announcement name is too long".to_string();
        return send_debug(ctx, err_str, why.to_string()).await;
    }

    let discord_name = match ctx.guild_id() {
        Some(guild_id) => match user.nick_in(&ctx, guild_id).await {
            Some(nick) => nick,
            None => user.name.clone(),
        },
        None => user.name.clone(),
    };

    let filename = format!("{}.flac", &announcement);
    let processing_path = "/config/processing/";

    let content = match file.download().await {
        Ok(content) => content,
        Err(why) => {
            let err_str = "Error downloading attachment".to_string();
            return send_error(ctx, err_str, why.to_string()).await;
        }
    };

    let mut file = match File::create(format!("{}{}", processing_path, &filename)) {
        Ok(file) => file,
        Err(why) => {
            let err_str = "Error creating file".to_string();
            return send_error(ctx, err_str, why.to_string()).await;
        }
    };

    if let Err(why) = file.write(&content) {
        let err_str = "Error writing file".to_string();
        return send_error(ctx, err_str, why.to_string()).await;
    }

    return add_new_file(ctx, &discord_name, &announcement, &user, filters.as_ref(), override_length).await;
}

#[doc = "Add new announcement using a url."]
#[poise::command(
    category = "Main Commands",
    guild_only,
    prefix_command,
    slash_command,
    required_bot_permissions = "SEND_MESSAGES"
)]
pub async fn url(
    ctx: PContext<'_>,
    #[description = "The user for which to add an announcement."] user: User,
    #[description = "Name of the announcement."] announcement: String,
    #[description = "URL for the announcement."] url: String,
    #[description = "Start time."] start: String,
    #[description = "End time."] end: String,
    #[description = "FFMPEG filters to transform audio."] filters: Option<String>,
    #[description = "Override the length limit"] override_length_limit: Option<bool>,
) -> Result<(), PError> {
    let mut override_length = false;
    if override_length_limit.is_some() {
        override_length = override_length_limit.unwrap();
        if override_length {
            if ctx.author().id.get() != BOT_ADMIN_USER_ID {
                let why = "".to_string();
                let err_str = "You are not allowed to use the length override!".to_string();
                return send_debug(ctx, err_str, why).await;
            }
        }
    }

    let discord_name = match ctx.guild_id() {
        Some(guild_id) => match user.nick_in(&ctx, guild_id).await {
            Some(nick) => nick,
            None => user.name.clone(),
        },
        None => user.name.clone(),
    };

    let filename = format!("{}.flac", &announcement);
    let processing_path = "/config/processing/";

    let _ = match Url::parse(&url) {
        Ok(url) => url,
        Err(why) => {
            let err_str = "Please provide a valid url".to_string();
            return send_debug(ctx, err_str, why.to_string()).await;
        }
    };

    let start_parsed = parse_duration(&start).unwrap();
    let end_parsed = parse_duration(&end).unwrap();
    let duration = end_parsed - start_parsed;

    if !override_length && duration > Duration::from_secs(7) {
        let why = duration.as_secs_f64();
        let err_str = "Duration is too long".to_string();
        return send_debug(ctx, err_str, why.to_string()).await;
    }

    let youtube_url = Command::new("yt-dlp")
        .arg("--no-playlist")
        .arg("-g")
        .arg(&url)
        .output()
        .expect("Failed to run youtube-dl");

    if !youtube_url.status.success() {
        let why = youtube_url.status;
        let errors = String::from_utf8(youtube_url.stderr).expect("Invalid error bytes");
        let err_str = format!("Youtube-dl Error: It likely needs an update, url = {}\n{}\n", &url, &why);
        return send_error(ctx, err_str, errors).await;
    }

    let youtube_dloutput = match String::from_utf8(youtube_url.stdout) {
        Ok(res) => res,
        Err(why) => {
            let err_str = "Failed to parse youtube-dl output".to_string();
            return send_error(ctx, err_str, why.to_string()).await;
        }
    };
    let lines = youtube_dloutput.lines();

    let audio_url = match lines.last() {
        Some(line) => line,
        None => {
            let why = url;
            let err_str = "Youtube empty info".to_string();
            return send_error(ctx, err_str, why.to_string()).await;
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
        .arg("flac")
        .arg(format!("file:{}", &filename))
        .current_dir(&processing_path)
        .output()
        .expect("failed to run ffmpeg")
        .status;

    if !download_status.success() {
        let why = download_status.code().expect("no exit code");
        let err_str = format!("Failed to run ffmpeg to download audio for file {}", &filename);
        return send_error(ctx, err_str, why.to_string()).await;
    }

    return add_new_file(ctx, &discord_name, &announcement, &user, filters.as_ref(), override_length).await;
}

pub async fn add_new_file(
    ctx: PContext<'_>,
    name: &String,
    announcement_name: &String,
    user: &User,
    filters: Option<&String>,
    override_length: bool,
) -> Result<(), PError> {
    let filename = format!("{}.flac", &announcement_name);
    let processed_filename = format!("{}{}", &announcement_name, ".processed.flac");
    let processing_path = "/config/processing/";
    let indexed_path = "/config/index/";
    let db_path = Path::new("/config/database/db.sqlite");

    let normalize_and_filter_string;
    if filters.is_some() {
        normalize_and_filter_string = format!("{},loudnorm", filters.unwrap());
    } else {
        normalize_and_filter_string = "loudnorm".to_string();
    }

    let mut args = vec!["-y"];
    if !override_length {
        args.push("-t");
        args.push("00:00:06");
    }

    let filter_output = Command::new("ffmpeg")
        .args(args)
        .arg("-i")
        .arg(format!("file:{}", &filename))
        .arg("-filter:a")
        .arg(&normalize_and_filter_string)
        .arg("-ar")
        .arg("48000")
        .arg("-f")
        .arg("flac")
        .arg(format!("file:{}", &processed_filename))
        .current_dir(&processing_path)
        .output()
        .expect("Failed to run ffmpeg");

    let mut ffmpeg_length_str = "";
    if !override_length {
        ffmpeg_length_str = " -t 00:00:06"
    }
    debug!(
        "ffmpeg -y{} -i {} -filter:a {} -ar 48000 -f flac {}",
        ffmpeg_length_str,
        format!("file:{}", &filename),
        &normalize_and_filter_string,
        format!("file:{}", &processed_filename)
    );

    if !filter_output.status.success() {
        let _ = delete_processing_files(&processing_path, &filename, &processed_filename);
        let why = filter_output.status.code().expect("no exit code");
        let err_str = format!("Failed to apply audio filter for file {}", &filename);
        return send_error(ctx, err_str, why.to_string()).await;
    }

    let name_path = format!("{}{}", &indexed_path, &name);

    if !Path::new(&name_path).exists() {
        let _ = fs::create_dir(name_path).expect("Failed to create directory.");
    }

    let db = match Connection::open(&db_path) {
        Ok(db) => db,
        Err(why) => {
            let err_str = "Failed to open database".to_string();
            return send_error(ctx, err_str, why.to_string()).await;
        }
    };

    let user_id = user.id.get();
    let insert_res = db.execute(
        "INSERT OR IGNORE INTO names (name, user_id, active_file)
              VALUES (?1, ?2, ?3) 
              ON CONFLICT(name, user_id) DO UPDATE SET
                active_file=excluded.active_file",
        params![&name, user_id as i64, announcement_name],
    );
    if insert_res.is_err() {
        let why = insert_res.err().unwrap();
        let err_str = "Failed to insert new name".to_string();
        return send_error(ctx, err_str, why.to_string()).await;
    };

    let _ = match fs::rename(
        format!("{}{}", &processing_path, &processed_filename),
        format!("{}{}/{}.flac", &indexed_path, &name, announcement_name),
    ) {
        Ok(res) => res,
        Err(why) => {
            let _ = delete_processing_files(&processing_path, &filename, &processed_filename);
            let err_str = format!("Failed to rename file {}", &processed_filename);
            return send_error(ctx, err_str, why.to_string()).await;
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

    let reply = CreateReply::default()
        .embed(CreateEmbed::new()
            .title(format!("Successfully added new file for {}", name))
            .description(format!("`{}` [{}]", &announcement_name, &user.mention()))
            .colour(Colour::from_rgb(128, 128, 128))
        );

    ctx.send(reply)
        .await
        .map(drop)
        .map_err(Into::into)
}

fn delete_processing_files(processing_path: &str, filename: &str, processed_filename: &str) {
    let _ = match fs::remove_file(format!("{}{}", &processing_path, &filename)) {
        Ok(res) => res,
        Err(why) => {
            debug!("Failed to remove queue file {}{} ERROR: {}", &processing_path, &filename, why);
        }
    };

    let _ = match fs::remove_file(format!("{}{}", &processing_path, &processed_filename)) {
        Ok(res) => res,
        Err(why) => {
            debug!("Failed to remove queue file {}{} ERROR: {}", &processing_path, &processed_filename, why);
        }
    };
}
