use std::{
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
    params,
};

use crate::lib::msg::check_msg;

const EMBED_DESCRIPTION_MAX_LENGTH: usize = 4096;

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
