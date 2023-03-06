use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

use serenity::{model::prelude::*, utils::Colour};

use crate::{util::util::{send_error, send_debug}, PContext, PError};

#[doc = "Toggle randomised mode."]
#[poise::command(
    category = "Main Commands",
    guild_only,
    prefix_command,
    slash_command,
    required_bot_permissions = "SEND_MESSAGES"
)]
pub async fn random(ctx: PContext<'_>) -> Result<(), PError> {
    let name = match ctx.author_member().await {
        Some(member) => member.display_name().into_owned(),
        None => ctx.author().name.clone(),
    };

    let db_path = Path::new("/config/database/db.sqlite");

    let db = match Connection::open(&db_path) {
        Ok(db) => db,
        Err(why) => {
            let err_str = format!("Failed to open database");
            return send_error(ctx, err_str, why.to_string()).await;
        }
    };

    let user_id = ctx.author().id.as_u64();
    let random_res = db
        .query_row::<i32, _, _>("SELECT random FROM names WHERE name=?1", params![&name], |row| {
            row.get(0)
        })
        .optional();

    if random_res.is_err() {
        let why = random_res.err().unwrap();
        let err_str = format!("Failed to query random status for {}", name);
        return send_error(ctx, err_str, why.to_string()).await;
    }
    if let Some(random_status) = random_res.unwrap() {
        let update_res = db.execute(
            "UPDATE names SET random = ((random | 1) - (random & 1)) WHERE name=?1 AND user_id=?2",
            params![&name, *user_id as i64],
        );
    
        if update_res.is_err() {
            let why = update_res.err().unwrap();
            let err_str = format!("Failed to random for name {}", &name);
            return send_error(ctx, err_str, why.to_string()).await;
        };
    
        let status_string;
        if random_status == 0 {
            status_string = "**ON**";
        } else {
            status_string = "**OFF**";
        }

        ctx.send(|m| {
            m.embed(|e| {
                e.title(format!("Toggled random {}", status_string))
                    .description(format!("{}", ctx.author().mention()))
                    .colour(Colour::from_rgb(128, 128, 128))
            })
        })
        .await
        .map(drop)
        .map_err(Into::into)
    } else {
        let why = name;
        let err_str = format!("Name doesn't exist");
        return send_debug(ctx, err_str, why.to_string()).await;
    }
}
