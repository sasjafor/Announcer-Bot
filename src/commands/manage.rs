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
};

use rusqlite::{
    Connection,
    OptionalExtension,
    params,
};

use lib::msg::check_msg;

#[command]
#[description("List all available announcements for a name")]
#[usage("<discordname>")]
#[example("Yzarul")]
#[example("\"Mr Yzarul\"")]
#[num_args(1)]
#[help_available]
pub fn list(ctx: &mut Context, message: &Message, args: Args) -> CommandResult {
    let arguments = args.raw_quoted().collect::<Vec<&str>>();

    let name = match arguments.first() {
        Some(name) => name,
        None => {
            check_msg(message.channel_id.say(&ctx, "Please provide a name"));
            return Ok(());
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

        check_msg(message.channel_id.say(&ctx, format!("Announcements for \"{}\"", &name)));

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
                if active_filename.is_some() && entry_value.path().file_stem().unwrap() == active_filename.unwrap() {
                    check_msg(message.channel_id.say(&ctx, format!("{}. {:?}   <--- ACTIVE FOR @{}", k, entry_value.path().file_stem().unwrap(), &message.author.name)));
                } else {
                    check_msg(message.channel_id.say(&ctx, format!("{}. {:?}", k, entry_value.path().file_stem().unwrap())));
                }
                k += 1;
            }
        }
    } else {
        check_msg(message.channel_id.say(&ctx, "This name doesn't exist"));
    }

    return Ok(());
}

#[command]
#[aliases("setactive", "set_active")]
#[description("Set the active announcement for a nickname")]
#[usage("<discordname> <announcement name>")]
#[example("Yzarul funny")]
#[example("\"Mr Yzarul\" \"funny noise\"")]
#[num_args(2)]
#[help_available]
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

    let path_string = format!("/config/index/{}/{}.wav", &name, &filename);
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

    check_msg(message.channel_id.say(&ctx, format!("Active file for user \"{}\" & name \"{}\" is now \"{}\"", &message.author.name, &name, &filename)));
    return Ok(());
}

#[command]
#[aliases("rand")]
#[description("Set random mode for a certain name")]
#[usage("<discordname> <random active>")]
#[example("Yzarul 1")]
#[example("\"Mr Yzarul\" 0")]
#[num_args(2)]
#[help_available]
pub fn random(ctx: &mut Context, message: &Message, args: Args) -> CommandResult {
    let arguments = args.raw_quoted().collect::<Vec<&str>>();

    let name = match arguments.first() {
        Some(name) => name,
        None => {
            check_msg(message.channel_id.say(&ctx, "Please provide a name"));
            return Ok(());
        }
    };

    let random = arguments[1];

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
        "INSERT OR REPLACE INTO names (name, user_id, random)
            VALUES (?1, ?2, ?3)",
        params![&name, *user_id as i64, random]) {
            Ok(_) => (),
            Err(err) => {
                error!("Failed to insert new name, Error Code {}", err);
                return Ok(());
            }
    };

    return Ok(());
}