use poise::CreateReply;
use rand::{distributions::Uniform, prelude::Distribution};
use rusqlite::{params, Connection, OptionalExtension};
use songbird::{input::File, tracks::Track};
use std::{
    fs::{self, read_dir},
    path::Path,
    process::Command,
};
use tracing::{debug, error, info, warn};

use serenity::{
    client::Context,
    model::id::{ChannelId, GuildId},
};

use crate::{PContext, PError};

pub async fn announce(ctx: &Context, channel_id: ChannelId, guild_id: GuildId, name: &str, user_id: u64) {
    let db_path = Path::new("/config/database/db.sqlite");

    let db = match Connection::open(&db_path) {
        Ok(db) => db,
        Err(err) => {
            error!("Failed to open database: {}", err);
            return;
        }
    };

    let filename = match db
        .query_row::<String, _, _>(
            "SELECT active_file FROM names WHERE name=?1 AND user_id=?2",
            params![&name, user_id as i64],
            |row| row.get(0),
        )
        .optional()
    {
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
            |row| row.get(0),
        ) {
            Ok(row) => row,
            Err(err) => {
                error!("Failed to query random file for {}, Error Code {}", name, err);
                return;
            }
        };

        let count = files.unwrap().count();
        if random && count > 0 {
            let between = Uniform::from(0..count);
            let mut rng = rand::thread_rng();
            let index = between.sample(&mut rng);

            let mut paths = read_dir(&index_base_path).unwrap();
            path = paths.nth(index).unwrap().unwrap().path().to_str().unwrap().to_owned();
        } else {
            path = format!("{}/{}.flac", &index_base_path, &filename.unwrap());
        }
    } else {
        path = format!("/config/audio/{}.flac", &name);
    }

    check_path(&path, &name);

    play_file(ctx, channel_id, guild_id, &path).await;
}

pub async fn play_file(ctx: &Context, channel_id: ChannelId, guild_id: GuildId, path: &str) {
    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let handler_lock = match manager.join(guild_id, channel_id).await {
        Ok(handler_lock) => handler_lock,
        Err(err) => {
            error!("Failed to connect to channel with id {} with err {}", channel_id, err);
            return;
        }
    };

    let mut handler = handler_lock.lock().await;

    let source = File::new(path.to_owned());
    let track = Track::from(source);

    info!("Playing sound file {}", path);
    handler.play(track);
}

pub async fn leave_channel(ctx: &Context, guild_id: GuildId) {
    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let handler_lock = match manager.get(guild_id) {
        Some(handler_lock) => handler_lock,
        None => {
            error!("Bot is not in a voice channel for guild: {}", guild_id);
            return;
        }
    };

    let mut handler = handler_lock.lock().await;
    let _ = handler.leave().await.expect("Failed to leave voice channel");
}

pub async fn bot_voice_channel_is_empty(ctx: &Context, guild_id: GuildId) -> bool {
    let mut is_empty = true;

    let guild = guild_id.to_guild_cached(&ctx).unwrap();

    let bot_voice_state = match guild.voice_states.get(&ctx.cache.current_user().id) {
        Some(voice_state) => voice_state,
        None => return false
    };

    let channel_id = match bot_voice_state.channel_id {
        Some(channel_id) => channel_id,
        None => return false
    };

    for state in guild
        .voice_states
        .values()
        .filter(|state| state.channel_id == Some(channel_id))
    {
        let user = match &state.member {
            Some(member) => &member.user,
            None => {
                error!("Error retrieving user from channel: {:?}", channel_id);
                continue;
            }
        };
        is_empty &= user.bot;
    }
    return is_empty;
}

pub fn check_path(path: &str, name: &str) {
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

        let _ = match fs::write(&text_path, name) {
            Ok(()) => (),
            Err(err) => {
                error!("Unable to write file {} for name: {} err: {}", &text_path, &name, &err);
                return;
            }
        };
    }
}

pub fn print_type_of<T>(_: &T) {
    println!("{}", std::any::type_name::<T>())
}

pub async fn send_debug(ctx: PContext<'_>, content: String, err: String) -> Result<(), PError> {
    debug!("{}: {}", content, err);

    let reply = CreateReply::default()
            .content(content)
            .ephemeral(true);

    ctx.send(reply)
        .await
        .map(drop)
        .map_err(Into::into)
}

pub async fn send_warning(ctx: PContext<'_>, content: String, err: String) -> Result<(), PError> {
    warn!("{}: {}", content, err);

    let reply = CreateReply::default()
            .content(content)
            .ephemeral(true);

    ctx.send(reply)
        .await
        .map(drop)
        .map_err(Into::into)
}

pub async fn send_error(ctx: PContext<'_>, content: String, err: String) -> Result<(), PError> {
    error!("{}: {}", content, err);

    let reply = CreateReply::default()
            .content(content)
            .ephemeral(true);

    ctx.send(reply)
        .await
        .map(drop)
        .map_err(Into::into)
}
