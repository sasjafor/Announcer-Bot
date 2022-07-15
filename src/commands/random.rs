use std::path::Path;

use serenity::{
    model::prelude::*,
    utils::Colour,
};

use tracing::error;

use rusqlite::{params, Connection};

use crate::{
    lib::{util::send_error},
    Error, PContext,
};

#[doc = "Toggle randomised mode."]
#[poise::command(
    category = "Main Commands",
    guild_only,
    prefix_command,
    slash_command,
    required_bot_permissions = "SEND_MESSAGES"
)]
pub async fn random(ctx: PContext<'_>) -> Result<(), Error> {
    let name = match ctx.author_member().await {
        Some(member) => member.display_name().into_owned(),
        None => ctx.author().name.clone(),
    };

    let db_path = Path::new("/config/database/db.sqlite");

    let db = match Connection::open(&db_path) {
        Ok(db) => db,
        Err(why) => {
            let err_str = format!("Failed to open database");
            error!("{}: {}", err_str, why);
            return send_error(ctx, err_str).await;
        }
    };

    let user_id = ctx.author().id.as_u64();
    let update_res = db.execute(
        "UPDATE names SET random = ((random | 1) - (random & 1)) WHERE name=?1 AND user_id=?2",
        params![&name, *user_id as i64],
    );

    if update_res.is_err() {
        let why = update_res.err().unwrap();
        let err_str = format!("Failed to random for name {}", &name);
        error!("{}: {}", err_str, why);
        return send_error(ctx, err_str).await;
    };

    ctx.send(|m| {
        m.embed(|e| {
            e.title(format!("Toggled random"))
                .description(format!("**{}** [{}]", &name, ctx.author().mention()))
                .colour(Colour::from_rgb(128, 128, 128))
        })
    })
    .await
    .map(drop)
    .map_err(Into::into)
}
