use std::{
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
};

use rusqlite::{
    params,
    Connection,
};

use lib::msg::check_msg;

#[command]
pub fn list(ctx: &mut Context, message: &Message, args: Args) -> CommandResult {
    let arguments = args.raw_quoted().collect::<Vec<&str>>();

    let name = match arguments.first() {
        Some(name) => name,
        None => {
            check_msg(message.channel_id.say(&ctx, "Please provide a name"));
            return Ok(());
        }
    };

    let path_string = format!("{}{}", "/config/index/", &name);
    let path = Path::new(&path_string);

    if path.is_dir() {
        let dir_iterator = match fs::read_dir(path) {
            Ok(dir_iterator) => dir_iterator,
            Err(err) => {
                error!("Failed to read directory {}, Error: {}", path_string, err);
                return Ok(());
            }
        };

        let mut k = 1;
        for entry in dir_iterator {
            let entry_value = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    error!("Failed to get next entry, Error: {}", err);
                    return Ok(());
                }
            };
            if !entry_value.path().is_dir() {
                check_msg(message.channel_id.say(&ctx, format!("{}{}{:?}", k, ". ", entry_value.path().file_stem().unwrap())));
                k += 1;
            }
        }
    } else {
        check_msg(message.channel_id.say(&ctx, "This name doesn't exist"));
    }

    return Ok(());
}

#[command]
pub fn set(ctx: &mut Context, message: &Message, args: Args) -> CommandResult {
    let arguments = args.raw_quoted().collect::<Vec<&str>>();

    let name = match arguments.first() {
        Some(name) => name,
        None => {
            check_msg(message.channel_id.say(&ctx, "Please provide a name"));
            return Ok(());
        }
    };

    if arguments.len() < 2 {
        check_msg(message.channel_id.say(&ctx, "Please provide the name for the file you want to set active"));
        return Ok(());
    }

    let filename = arguments[1];

    let path_string = format!("{}{}{}{}{}", "/config/index/", &name, "/", &filename, ".wav");
    let path = Path::new(&path_string);

    if !path.exists() {
        check_msg(message.channel_id.say(&ctx, "Please choose a valid filename"));
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

    let _ = match db.execute(
        "INSERT OR REPLACE INTO names (name, active_file)
            VALUES (?1, ?2)",
        params![&name, &filename]) {
            Ok(_) => (),
            Err(err) => {
                error!("Failed to insert new name, Error Code {}", err);
                return Ok(());
            }
    };

    check_msg(message.channel_id.say(&ctx, format!("Active file for {} is now {}", &name, &filename)));
    return Ok(());
}