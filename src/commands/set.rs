use crate::{
    lib::{component_ids::*, util::send_error},
    pContext, Error,
};

use std::{fs, path::Path, time::Duration};

use poise::{futures_util::StreamExt, serenity_prelude::CreateComponents};
use serenity::{builder::CreateSelectMenuOption, model::prelude::*, utils::Colour};

use rusqlite::{params, Connection};
use tracing::{debug, error, warn};

const TIMEOUT_DURATION: Duration = Duration::from_secs(300);
const ELEMENTS_PER_MENU: usize = 25;
const ELEMENT_LABEL_LENGTH: usize = 100;

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
        let mut index = 1;
        let (options, over_limit) = match create_dropdown_options(ctx, &discord_name, index).await {
            Ok(res) => res,
            Err(why) => return Err(why),
        };

        let message = ctx
            .send(|m| {
                m.content("Choose an announcement").components(|c| {
                    create_dropdown(c, false, over_limit, options);
                    c
                })
            })
            .await?
            .message()
            .await
            .unwrap();

        let mut collector = message.await_component_interactions(ctx.discord()).timeout(TIMEOUT_DURATION).build();
        while let Some(interaction) = collector.next().await {
            match interaction.data.custom_id.as_str() {
                ANNOUNCEMENT_SELECTOR_DROPDOWN => {
                    let announcement_name = interaction.data.values.first().unwrap().to_owned();
                    return match set_fn(ctx, &announcement_name, &discord_user, &discord_name).await {
                        Ok(_) => interaction
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
                            .map_err(Into::into),
                        Err(why) => Err(why),
                    };
                }
                ANNOUNCEMENT_SELECTOR_PREV_BUTTON => {
                    index -= 1;
                }
                ANNOUNCEMENT_SELECTOR_NEXT_BUTTON => {
                    index += 1;
                }
                _ => {
                    let err_str = "Unknown component interaction".to_string();
                    debug!("{}", err_str);
                    return send_error(ctx, err_str).await;
                }
            }

            let (options, over_limit) = match create_dropdown_options(ctx, &discord_name, index).await {
                Ok(res) => res,
                Err(why) => return Err(why),
            };
            if let Err(why) = interaction
                .create_interaction_response(&ctx.discord().http, |response| {
                    response
                        .kind(InteractionResponseType::UpdateMessage)
                        .interaction_response_data(|m| {
                            m.components(|c| {
                                create_dropdown(c, index > 1, over_limit, options);
                                c
                            })
                        })
                })
                .await
            {
                return Err(Into::into(why));
            }
        }

        let err_str = "Failed to wait for component interaction".to_string();
        warn!("{}", err_str);
        return send_error(ctx, err_str).await;
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

async fn create_dropdown_options(
    ctx: pContext<'_>,
    discord_name: &String,
    index: usize,
) -> Result<(Vec<CreateSelectMenuOption>, bool), Error> {
    let mut options = vec![];
    let mut over_limit = false;

    let path_string = format!("/config/index/{}", &discord_name);
    let path = Path::new(&path_string);
    if path.is_dir() {
        let dir_iterator = match fs::read_dir(path) {
            Ok(dir_iterator) => dir_iterator,
            Err(err) => {
                let err_str = format!("Failed to read directory {}", path_string);
                error!("{}: {}", err_str, err);
                return match send_error(ctx, err_str).await {
                    Ok(_) => Err(Into::into(err)),
                    Err(why) => Err(why),
                };
            }
        };

        let mut count = 0;
        let start_pos = (index - 1) * ELEMENTS_PER_MENU;
        for entry in dir_iterator {
            // skip start elements
            if count < start_pos {
                count += 1;
                continue;
            }

            let entry_value = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    let err_str = "Failed to get next entry".to_string();
                    error!("{}: {}", err_str, err);
                    return match send_error(ctx, err_str.clone()).await {
                        Ok(_) => Err(Into::into(err)),
                        Err(why) => Err(why),
                    };
                }
            };
            let announcement_name = entry_value.path().file_stem().unwrap().to_string_lossy().to_string();
            // limit length
            if announcement_name.chars().count() > ELEMENT_LABEL_LENGTH {
                let err_str = format!("Announcement name is too long {}", announcement_name);
                warn!("{}: {}", err_str, announcement_name.len());
                return match send_error(ctx, err_str.clone()).await {
                    Ok(_) => Err(Into::into(serenity::Error::Other("Err"))),
                    Err(why) => Err(why),
                };
            }

            let announcement: CreateSelectMenuOption = CreateSelectMenuOption::default()
                .label(announcement_name.clone())
                .value(announcement_name)
                .to_owned();
            options.push(announcement);

            count += 1;
            if count - start_pos >= ELEMENTS_PER_MENU {
                over_limit = true;
                break;
            }
        }
    }

    return Ok((options, over_limit));
}

fn create_dropdown(
    components: &mut CreateComponents,
    prev: bool,
    next: bool,
    options: Vec<CreateSelectMenuOption>,
) -> () {
    components
        .create_action_row(|r| {
            r.create_select_menu(|s| {
                s.custom_id(ANNOUNCEMENT_SELECTOR_DROPDOWN)
                    .min_values(1)
                    .max_values(1)
                    .options(|c| c.set_options(options))
            })
        })
        .create_action_row(|r| {
            r.create_button(|b| {
                b.custom_id(ANNOUNCEMENT_SELECTOR_PREV_BUTTON).label("Prev");
                if !prev {
                    b.disabled(true);
                }
                b
            })
            .create_button(|b| {
                b.custom_id(ANNOUNCEMENT_SELECTOR_NEXT_BUTTON).label("Next");
                if !next {
                    b.disabled(true);
                }
                b
            })
        });
}
