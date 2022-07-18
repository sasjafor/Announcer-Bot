use rusqlite::{params, Connection};
use std::{fs, path::Path, time::Duration};
use tracing::debug;

use poise::futures_util::StreamExt;
use serenity::{
    builder::{CreateComponents, CreateSelectMenuOption},
    model::prelude::*,
    utils::Colour,
};

use crate::{
    lib::{
        component_ids::*,
        util::{send_debug, send_error, send_warning}, consts::{ELEMENTS_PER_MENU, ELEMENT_LABEL_LENGTH},
    },
    PContext, PError,
};

const TIMEOUT_DURATION: Duration = Duration::from_secs(300);

#[doc = "Set the active announcement for the current nickname."]
#[poise::command(
    category = "Main Commands",
    guild_only,
    prefix_command,
    slash_command,
    required_bot_permissions = "SEND_MESSAGES"
)]
pub async fn set(
    ctx: PContext<'_>,
    #[description = "The name for which to set the announcement."] name: Option<String>,
    #[description = "The user for which to set the active announcement."] user: Option<User>,
) -> Result<(), PError> {
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
                m.content("Choose an announcement");
                    m.components(|c| {
                        create_dropdown(c, false, over_limit, options);
                        c
                    });
                m
            })
            .await?
            .message()
            .await
            .unwrap();

        let mut collector = message
            .await_component_interactions(ctx.discord())
            .timeout(TIMEOUT_DURATION)
            .build();
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
                    let why = &interaction.data.custom_id;
                    let err_str = "Unknown component interaction".to_string();
                    return send_warning(ctx, err_str, why.to_string()).await;
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

        let err_str = "Timeout".to_string();
        debug!("{}", err_str);
        let _ = message.delete(&ctx.discord().http).await;
        return Ok(());
    }
}

async fn set_fn(
    ctx: PContext<'_>,
    announcement_name: &String,
    discord_user: &User,
    discord_name: &String,
) -> Result<(), PError> {
    let path_string = format!("/config/index/{}/{}.wav", discord_name, announcement_name);
    let path = Path::new(&path_string);

    if !path.exists() {
        let why = "File doesn't exist.";
        let err_str = "Please choose a valid announcement".to_string();
        return send_debug(ctx, err_str, why.to_string()).await;
    }

    let db_path = Path::new("/config/database/db.sqlite");

    let db = match Connection::open(&db_path) {
        Ok(db) => db,
        Err(why) => {
            let err_str = "Failed to open database".to_string();
            return send_error(ctx, err_str, why.to_string()).await;
        }
    };

    let user_id = discord_user.id.as_u64();
    let insert_res = db.execute(
        "INSERT OR REPLACE INTO names (name, user_id, active_file)
            VALUES (?1, ?2, ?3)",
        params![discord_name, *user_id as i64, announcement_name],
    );

    if insert_res.is_err() {
        let why = insert_res.err().unwrap();
        let err_str = "Failed to insert new name".to_string();
        return send_error(ctx, err_str, why.to_string()).await;
    };

    Ok(())
}

async fn create_dropdown_options(
    ctx: PContext<'_>,
    discord_name: &String,
    index: usize,
) -> Result<(Vec<CreateSelectMenuOption>, bool), PError> {
    let mut options = vec![];
    let mut over_limit = false;

    let path_string = format!("/config/index/{}", &discord_name);
    let path = Path::new(&path_string);
    if path.is_dir() {
        let dir_iterator = match fs::read_dir(path) {
            Ok(dir_iterator) => dir_iterator,
            Err(why) => {
                let err_str = format!("Failed to read directory {}", path_string);
                return match send_error(ctx, err_str, why.to_string()).await {
                    Ok(_) => Err(Into::into(why)),
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
                Err(why) => {
                    let err_str = "Failed to get next entry".to_string();
                    return match send_error(ctx, err_str, why.to_string()).await {
                        Ok(_) => Err(Into::into(why)),
                        Err(why) => Err(why),
                    };
                }
            };
            let announcement_name = entry_value.path().file_stem().unwrap().to_string_lossy().to_string();
            // limit length
            if announcement_name.chars().count() > ELEMENT_LABEL_LENGTH {
                let why = announcement_name.len();
                let err_str = format!("Announcement name is too long {}", announcement_name);
                return match send_warning(ctx, err_str, why.to_string()).await {
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
        });
        if prev || next {
            components.create_action_row(|r| {
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
    };
}
