use rusqlite::{params, Connection};
use std::{fs, fs::File, io::prelude::*, path::Path, process::Command, time::Duration};
use tracing::debug;
use url::Url;

use serenity::{model::prelude::*, utils::Colour};

use crate::{
    lib::{
        parse::parse_duration,
        util::{send_debug, send_error, send_warning}, consts::ELEMENT_LABEL_LENGTH,
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
    explanation_fn = "new_help"
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
) -> Result<(), PError> {
    let announcement_length = announcement.chars().count();
    if announcement_length > ELEMENT_LABEL_LENGTH {
        let why = announcement_length;
        let err_str = "Error creating file".to_string();
        return send_warning(ctx, err_str, why.to_string()).await;
    }

    let discord_name = match ctx.guild_id() {
        Some(guild_id) => match user.nick_in(&ctx.discord().http, guild_id).await {
            Some(nick) => nick,
            None => user.name.clone(),
        },
        None => user.name.clone(),
    };

    let filename = format!("{}.wav", &announcement);
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

    return add_new_file(ctx, &discord_name, &announcement, &user, filters.as_ref()).await;
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
) -> Result<(), PError> {
    let discord_name = match ctx.guild_id() {
        Some(guild_id) => match user.nick_in(&ctx.discord().http, guild_id).await {
            Some(nick) => nick,
            None => user.name.clone(),
        },
        None => user.name.clone(),
    };

    let filename = format!("{}.wav", &announcement);
    let processing_path = "/config/processing/";

    let _ = match Url::parse(&url) {
        Ok(url) => url,
        Err(why) => {
            let err_str = "Please provide a valid url".to_string();
            return send_error(ctx, err_str, why.to_string()).await;
        }
    };

    let start_parsed = parse_duration(&start).unwrap();
    let end_parsed = parse_duration(&end).unwrap();
    let duration = end_parsed - start_parsed;

    if duration > Duration::from_secs(7) {
        let why = duration.as_secs_f64();
        let err_str = "Duration is too long".to_string();
        return send_debug(ctx, err_str, why.to_string()).await;
    }

    let youtube_url = Command::new("youtube-dl")
        .arg("-g")
        .arg(&url)
        .output()
        .expect("Failed to run youtube-dl");

    if !youtube_url.status.success() {
        let why = youtube_url.status;
        let err_str = format!("Youtube-dl Error: It likely needs an update, url = {}", &url);
        return send_error(ctx, err_str, why.to_string()).await;
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
        .arg("wav")
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

    return add_new_file(ctx, &discord_name, &announcement, &user, filters.as_ref()).await;
}

pub async fn add_new_file(
    ctx: PContext<'_>,
    name: &String,
    announcement_name: &String,
    user: &User,
    filters: Option<&String>,
) -> Result<(), PError> {
    let filename = format!("{}.wav", &announcement_name);
    let processed_filename = format!("{}{}", &announcement_name, ".processed.wav");
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

    debug!(
        "ffmpeg -y -t 00:00:06 -i {} -filter:a {} -ar 48000 -f wav {}",
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

    let user_id = user.id.as_u64();
    let insert_res = db.execute(
        "INSERT OR REPLACE INTO names (name, user_id, active_file)
            VALUES (?1, ?2, ?3)",
        params![&name, *user_id as i64, announcement_name],
    );
    if insert_res.is_err() {
        let why = insert_res.err().unwrap();
        let err_str = "Failed to insert new name".to_string();
        return send_error(ctx, err_str, why.to_string()).await;
    };

    let _ = match fs::rename(
        format!("{}{}", &processing_path, &processed_filename),
        format!("{}{}/{}.wav", &indexed_path, &name, announcement_name),
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

    ctx.send(|m| {
        m.embed(|e| {
            e.title(format!("Successfully added new file for {}", name))
                .description(format!("`{}` [{}]", &announcement_name, &user.mention()))
                .colour(Colour::from_rgb(128, 128, 128))
        })
    })
    .await
    .map(drop)
    .map_err(Into::into)
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
