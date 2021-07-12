use std::{
    ffi::OsStr,
    fs, 
    path::Path, 
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
    utils::Colour,
};

use tracing::{error};

use rusqlite::{
    Connection,
    OptionalExtension,
    params,
};

use crate::lib::msg::check_msg;

const EMBED_DESCRIPTION_MAX_LENGTH: usize = 4096;

#[command]
#[description("List all available announcements for a name")]
#[usage("<discordname>")]
#[example("")]
#[example("Yzarul")]
#[example("\"Mr Yzarul\"")]
#[min_args(0)]
#[max_args(1)]
#[help_available]
pub async fn list(ctx: &Context, message: &Message, args: Args) -> CommandResult {
    let arguments = args.raw_quoted().collect::<Vec<&str>>();

    let option_nick = &message.author_nick(&ctx).await;
    let name = match arguments.first() {
        Some(name) => *name,
        None => {
            match option_nick {
                Some(nick) => nick,
                None => &message.author.name
            }
        }
    };

    let path_string = format!("/config/index/{}", &name);
    let path = Path::new(&path_string);

    let db_path = Path::new("/config/database/db.sqlite");

    let db = match Connection::open(&db_path) {
        Ok(db) => db,
        Err(err) => {
            error!("Failed to open database: {}", err);
            return Ok(());
        }
    };

    let filename = match db.query_row::<String, _, _>(
        "SELECT active_file FROM names WHERE name=?1",
        params![&name],
        |row| row.get(0)).optional() {
            Ok(filename) => filename,
            Err(err) => {
                error!("Failed to query active file for {}, Error Code {}", name, err);
                return Ok(());
            }
    };

    if path.is_dir() {
        let dir_iterator = match fs::read_dir(path) {
            Ok(dir_iterator) => dir_iterator,
            Err(err) => {
                error!("Failed to read directory {}, Error: {}", path_string, err);
                return Ok(());
            }
        };

        let mut active_filename = None;
        let active_filename_str;
        if filename.is_some() {
            active_filename_str = filename.unwrap();
            active_filename = Some(OsStr::new(&active_filename_str));
        }

        let mut msg_len = 0;
        let mut msg_str = "".to_string();
        for entry in dir_iterator {
            let entry_value = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    error!("Failed to get next entry, Error: {}", err);
                    return Ok(());
                }
            };
            let mut line_str = "".to_owned();
            if !entry_value.path().is_dir() {
                if active_filename.is_some() && entry_value.path().file_stem().unwrap() == active_filename.unwrap() {
                    line_str = format!("• `{}` <=={:=>30}", entry_value.path().file_stem().unwrap().to_str().unwrap(), format!(" {}", &message.author.mention()));
                } else {
                    line_str = format!("• `{}`", entry_value.path().file_stem().unwrap().to_str().unwrap());
                }
            }

            let line_len = line_str.chars().count();
            if msg_len + line_len > EMBED_DESCRIPTION_MAX_LENGTH {
                let msg_res = message.channel_id.send_message(&ctx, |m| {
                    m.embed(|e| {
                        e.title(format!("Announcements for \"{}\"", &name));
                        e.description(msg_str);
                        e.colour(Colour::from_rgb(128,128,128));
            
                        e
                    });
            
                    m
                });
                check_msg(msg_res.await);

                msg_str = "".to_string();
                msg_len = 0;
            }

            msg_str.push_str("\n");
            msg_str.push_str(&line_str);
            msg_len += line_len + 1;
        }

        let msg_res = message.channel_id.send_message(&ctx, |m| {
            m.embed(|e| {
                e.title(format!("Announcements for \"{}\"", &name));
                e.description(msg_str);
                e.colour(Colour::from_rgb(128,128,128));
    
                e
            });
    
            m
        });
        check_msg(msg_res.await);
    } else {
        check_msg(message.channel_id.say(&ctx, "This name doesn't exist").await);
    }

    return Ok(());
}

#[command]
#[aliases("setactive", "set_active")]
#[description("Set the active announcement for the current nickname")]
#[usage("<announcement name>")]
#[example("funny")]
#[example("\"funny noise\"")]
#[num_args(1)]
#[help_available]
pub async fn set(ctx: &Context, message: &Message, args: Args) -> CommandResult {
    let arguments = args.raw_quoted().collect::<Vec<&str>>();

    let option_nick = &message.author_nick(&ctx).await;
    let name = match option_nick {
        Some(nick) => nick,
        None => &message.author.name
    };

    if arguments.len() < 1 {
        check_msg(message.channel_id.say(&ctx, "Please provide the name for the file you want to set active").await);
        return Ok(());
    }

    let filename = arguments[0];

    let path_string = format!("/config/index/{}/{}.wav", &name, &filename);
    let path = Path::new(&path_string);

    if !path.exists() {
        check_msg(message.channel_id.say(&ctx, "Please choose a valid filename").await);
        return Ok(());
    }

    let db_path = Path::new("/config/database/db.sqlite");

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
        params![&name, *user_id as i64, &filename]) {
            Ok(_) => (),
            Err(err) => {
                error!("Failed to insert new name, Error Code {}", err);
                return Ok(());
            }
    };

    let msg_res = message.channel_id.send_message(&ctx, |m| {
        m.embed(|e| {
            e.title(format!("Set announcement"));
            e.description(format!("`{}` [{}]", &filename, &message.author.mention()));
            e.colour(Colour::from_rgb(128,128,128));

            e
        });

        m
    });
    check_msg(msg_res.await);
    return Ok(());
}

#[command]
#[aliases("rand")]
#[description("Toggle random mode")]
#[usage("")]
#[example("")]
#[num_args(0)]
#[help_available]
pub async fn random(ctx: &Context, message: &Message, _args: Args) -> CommandResult {
    let option_nick = &message.author_nick(&ctx).await;
    let name = match option_nick {
        Some(nick) => nick,
        None => &message.author.name
    };

    let db_path = Path::new("/config/database/db.sqlite");

    let db = match Connection::open(&db_path) {
        Ok(db) => db,
        Err(err) => {
            error!("Failed to open database: {}", err);
            return Ok(());
        }
    };

    let user_id = message.author.id.as_u64();
    let _ = match db.execute(
        "UPDATE names SET random = ((random | 1) - (random & 1)) WHERE name=?1 AND user_id=?2",
        params![&name, *user_id as i64]) {
            Ok(_) => (),
            Err(err) => {
                error!("Failed to random for name {}, Error Code {}", &name, err);
                return Ok(());
            }
    };

    let msg_res = message.channel_id.send_message(&ctx, |m| {
        m.embed(|e| {
            e.title(format!("Toggled random"));
            e.description(format!("**{}** [{}]", &name, &message.author.mention()));
            e.colour(Colour::from_rgb(128,128,128));

            e
        });

        m
    });
    check_msg(msg_res.await);
    return Ok(());
}