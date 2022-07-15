use rand::{distributions::Uniform, prelude::Distribution};
use rusqlite::{params, Connection, OptionalExtension};
use std::{
    fs::{self, read_dir},
    path::Path,
    process::Command,
};
use tracing::{debug, error, info, warn};

use serenity::{
    client::Context,
    model::{
        guild::Guild,
        id::{ChannelId, GuildId},
    },
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
            path = format!("{}/{}.wav", &index_base_path, &filename.unwrap());
        }
    } else {
        path = format!("/config/audio/{}.wav", &name);
    }

    check_path(&path, &name);

    play_file(ctx, channel_id, guild_id, &path).await;
}

pub async fn play_file(ctx: &Context, channel_id: ChannelId, guild_id: GuildId, path: &str) {
    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let handler_lock = manager.get_or_insert(songbird::id::GuildId(guild_id.0));
    let mut handler = handler_lock.lock().await;

    let songbird_channel_id = songbird::id::ChannelId(channel_id.0);
    if handler.current_channel().is_none() || handler.current_channel().unwrap() != songbird_channel_id {
        let handler_res = match handler.join(songbird_channel_id).await {
            Ok(res) => res,
            Err(err) => {
                error!(
                    "Failed to send connect request for channel with id {} with err {}",
                    channel_id, err
                );
                return;
            }
        };
        drop(handler);
        let _ = match handler_res.await {
            Ok(_res) => _res,
            Err(err) => {
                error!("Failed to connect to channel with id {} with err {}", channel_id, err);
                return;
            }
        };

        handler = handler_lock.lock().await;
    }

    let source = match songbird::ffmpeg(path).await {
        Ok(source) => source,
        Err(err) => {
            error!("Err starting source: {:?}", err);
            return;
        }
    };

    info!("Playing sound file {}", path);
    handler.play_source(source);
}

pub async fn voice_channel_is_empty(ctx: &Context, guild: &Guild, channel_id: ChannelId) -> bool {
    let mut is_empty = true;
    for state in guild
        .voice_states
        .values()
        .filter(|state| state.channel_id == Some(channel_id))
    {
        let user = match state.user_id.to_user(ctx).await {
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
    ctx.send(|m| m.content(content).ephemeral(true))
        .await
        .map(drop)
        .map_err(Into::into)
}

pub async fn send_warning(ctx: PContext<'_>, content: String, err: String) -> Result<(), PError> {
    warn!("{}: {}", content, err);
    ctx.send(|m| m.content(content).ephemeral(true))
        .await
        .map(drop)
        .map_err(Into::into)
}

pub async fn send_error(ctx: PContext<'_>, content: String, err: String) -> Result<(), PError> {
    error!("{}: {}", content, err);
    ctx.send(|m| m.content(content).ephemeral(true))
        .await
        .map(drop)
        .map_err(Into::into)
}
