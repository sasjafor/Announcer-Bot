use crate::{lib::util::send_error, pContext, Error};

use std::path::Path;

use serenity::{model::prelude::*, utils::Colour};

use rusqlite::{params, Connection};
use tracing::{debug, error};

#[doc = "Set the active announcement for the current nickname."]
#[poise::command(
    category = "Main Commands",
    guild_only,
    prefix_command,
    slash_command,
    required_bot_permissions = "SEND_MESSAGES"
)]
pub async fn set(
    ctx: pContext<'_>,
    #[description = "The name for which to set the announcement."] name: String,
    #[description = "The user for which to set the active announcement."] user: Option<User>,
) -> Result<(), Error> {
    let announcement_name = name;
    let discord_user = match user {
        Some(user) => user,
        None => ctx.author().clone(),
    };
    let discord_name = match ctx.guild_id() {
        Some(guild_id) => match discord_user.nick_in(&ctx.discord().http, guild_id).await {
            Some(nick) => nick,
            None => discord_user.name.clone(),
        },
        None => discord_user.name.clone(),
    };

    let path_string = format!("/config/index/{}/{}.wav", &discord_name, &announcement_name);
    let path = Path::new(&path_string);

    if !path.exists() {
        let err_str = "Please choose a valid announcement".to_string();
        debug!("{}: {}", err_str, "File doesn't exist.");
        return send_error(ctx, err_str).await;
    }

    let db_path = Path::new("/config/database/db.sqlite");

    let db = match Connection::open(&db_path) {
        Ok(db) => db,
        Err(why) => {
            let err_str = "Failed to open database".to_string();
            error!("{}: {}", err_str, why);
            return send_error(ctx, err_str).await;
        }
    };

    let user_id = discord_user.id.as_u64();
    let insert_res = db.execute(
        "INSERT OR REPLACE INTO names (name, user_id, active_file)
            VALUES (?1, ?2, ?3)",
        params![&discord_name, *user_id as i64, &announcement_name],
    ); 
    
    if insert_res.is_err() {
        let err_str = "Failed to insert new name".to_string();
        error!("{}: {}", err_str, insert_res.err().unwrap());
        return send_error(ctx, err_str).await;
    };

    ctx.send(|m| {
        m.embed(|e| {
            e.title(format!("Set announcement"))
                .description(format!("`{}` [{}]", &announcement_name, &discord_user.mention()))
                .colour(Colour::from_rgb(128, 128, 128))
        })
    })
    .await
    .map(drop)
    .map_err(Into::into)
}
