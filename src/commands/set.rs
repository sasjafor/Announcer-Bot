use crate::{
    lib::{component_ids::announcement_selector_dropdown, util::send_error},
    pContext, Error,
};

use std::{fs, path::Path};

use serenity::{builder::CreateSelectMenuOption, model::prelude::*, utils::Colour};

use rusqlite::{params, Connection};
use tracing::{debug, error, warn};

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
    #[description = "The name for which to set the announcement."] name: Option<String>,
    #[description = "The user for which to set the active announcement."] user: Option<User>,
) -> Result<(), Error> {
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

    if let Some(announcement_name) = name {
        match set_fn(ctx, &announcement_name, &discord_user, &discord_name).await {
            Ok(_) => ctx
                .send(|m| {
                    m.embed(|e| {
                        e.title(format!("Set announcement"))
                            .description(format!("`{}` [{}]", &announcement_name, &discord_user.mention()))
                            .colour(Colour::from_rgb(128, 128, 128))
                    })
                })
                .await
                .map(drop)
                .map_err(Into::into),
            Err(why) => Err(why),
        }
    } else {
        let mut options = vec![];

        let path_string = format!("/config/index/{}", &discord_name);
        let path = Path::new(&path_string);
        if path.is_dir() {
            let dir_iterator = match fs::read_dir(path) {
                Ok(dir_iterator) => dir_iterator,
                Err(err) => {
                    let err_str = format!("Failed to read directory {}", path_string);
                    error!("{}: {}", err_str, err);
                    return send_error(ctx, err_str).await;
                }
            };

            for entry in dir_iterator {
                let entry_value = match entry {
                    Ok(entry) => entry,
                    Err(err) => {
                        let err_str = "Failed to get next entry".to_string();
                        error!("{}: {}", err_str, err);
                        return send_error(ctx, err_str).await;
                    }
                };
                let announcement_name = entry_value.path().file_stem().unwrap().to_string_lossy().to_string();

                let announcement: CreateSelectMenuOption = CreateSelectMenuOption::default()
                    .label(announcement_name.clone())
                    .value(announcement_name)
                    .to_owned();
                options.push(announcement);
            }
        }

        let message = ctx
            .send(|m| {
                m.content("Choose an announcement").components(|components| {
                    components.create_action_row(|r| {
                        r.create_select_menu(|s| {
                            s.custom_id(announcement_selector_dropdown)
                                .min_values(1)
                                .max_values(1)
                                .options(|c| c.set_options(options))
                        })
                    })
                })
            })
            .await?
            .message()
            .await
            .unwrap();

        let interaction = message.await_component_interaction(ctx.discord()).await;
        if let Some(interaction) = interaction {
            match interaction.data.custom_id.as_str() {
                announcement_selector_dropdown => {
                    let announcement_name = interaction.data.values.first().unwrap().to_owned();
                    match set_fn(ctx, &announcement_name, &discord_user, &discord_name).await {
                        Ok(_) => {
                            interaction
                                .create_interaction_response(&ctx.discord().http, |response| {
                                    response
                                        .kind(InteractionResponseType::UpdateMessage)
                                        .interaction_response_data(|m| {
                                            m.content("")
                                            .embed(|e| {
                                                e.title(format!("Set announcement"))
                                                    .description(format!(
                                                        "`{}` [{}]",
                                                        &announcement_name,
                                                        &discord_user.mention()
                                                    ))
                                                    .colour(Colour::from_rgb(128, 128, 128))
                                            })
                                            .components(|c| c)
                                        })
                                })
                                .await
                                .map(drop)
                                .map_err(Into::into)
                        }
                        Err(why) => Err(why),
                    }
                }
                _ => {
                    let err_str = "Unknown component interaction".to_string();
                    debug!("{}", err_str);
                    return send_error(ctx, err_str).await;
                }
            }
        } else {
            let err_str = "Failed to wait for component interaction".to_string();
            warn!("{}", err_str);
            return send_error(ctx, err_str).await;
        }
    }
}

async fn set_fn(
    ctx: pContext<'_>,
    announcement_name: &String,
    discord_user: &User,
    discord_name: &String,
) -> Result<(), Error> {
    let path_string = format!("/config/index/{}/{}.wav", discord_name, announcement_name);
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
        params![discord_name, *user_id as i64, announcement_name],
    );

    if insert_res.is_err() {
        let err_str = "Failed to insert new name".to_string();
        error!("{}: {}", err_str, insert_res.err().unwrap());
        return send_error(ctx, err_str).await;
    };

    Ok(())
}
