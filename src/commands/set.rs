use rusqlite::{params, Connection};
use std::{fs, path::Path, time::Duration};
use tracing::debug;

use poise::{futures_util::StreamExt, CreateReply};
use serenity::{
    all::{
        CreateActionRow, 
        CreateEmbed, 
        CreateInteractionResponse, 
        CreateInteractionResponseMessage, 
        CreateSelectMenu, 
        CreateSelectMenuKind
    }, 
    builder::CreateSelectMenuOption, 
    model::{
        colour::Colour,
        prelude::*,
    }
};

use crate::{
    util::{
        component_ids::*, consts::{ELEMENTS_PER_MENU, ELEMENT_LABEL_LENGTH}, messages::create_navigation_buttons, util::{send_debug, send_error, send_warning}
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
        Some(guild_id) => match discord_user.nick_in(&ctx, guild_id).await {
            Some(nick) => nick,
            None => discord_user.name.clone(),
        },
        None => discord_user.name.clone(),
    };

    if let Some(announcement_name) = name {
        match set_fn(ctx, &announcement_name, &discord_user, &discord_name).await {
            Ok(_) => {
                let reply = CreateReply::default()
                    .embed(CreateEmbed::new()
                        .title(format!("Set announcement"))
                        .description(format!("`{}` [{}]", &announcement_name, &discord_user.mention()))
                        .colour(Colour::from_rgb(128, 128, 128))
                    );

                ctx
                    .send(reply)
                    .await
                    .map(drop)
                    .map_err(Into::into)
            },
            Err(why) => Err(why),
        }
    } else {
        let mut index = 1;
        let (options, over_limit) = match create_dropdown_options(ctx, &discord_name, index).await {
            Ok(res) => res,
            Err(why) => return Err(why),
        };

        let reply = CreateReply::default()
            .content("Choose an announcement")
            .components(create_dropdown(false, over_limit, options));

        let message = ctx
            .send(reply)
                .await?
                .into_message()
                .await
                .unwrap();

        let mut collector = message
            .await_component_interactions(ctx)
            .timeout(TIMEOUT_DURATION)
            .stream();
        while let Some(interaction) = collector.next().await {
            match interaction.data.custom_id.as_str() {
                ANNOUNCEMENT_SELECTOR_DROPDOWN => {
                    let announcement_name;
                    if let ComponentInteractionDataKind::StringSelect { values } = &interaction.data.kind {
                        announcement_name = values.first().unwrap().to_owned();
                    } else {
                        let why = &interaction.data.custom_id;
                        let err_str = "No select menu data was found for".to_string();
                        return send_warning(ctx, err_str, why.to_string()).await;
                    }
                    return match set_fn(ctx, &announcement_name, &discord_user, &discord_name).await {
                        Ok(_) => {
                            let interaction_response = CreateInteractionResponseMessage::default()
                                .embed(CreateEmbed::new()
                                    .title(format!("Set announcement"))
                                    .description(format!(
                                        "`{}` [{}]",
                                        &announcement_name,
                                        &discord_user.mention()
                                    ))
                                    .colour(Colour::from_rgb(128, 128, 128))
                                );

                            interaction
                                .create_response(
                                    &ctx, 
                                    CreateInteractionResponse::UpdateMessage(interaction_response)
                                )
                                .await
                                .map(drop)
                                .map_err(Into::into)
                        },
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

            let interaction_response = CreateInteractionResponseMessage::default()
                .components(create_dropdown(index > 1, over_limit, options));

            if let Err(why) = interaction
                .create_response(
                    &ctx,
                    CreateInteractionResponse::UpdateMessage(interaction_response)
                )
                .await
            {
                return Err(Into::into(why));
            }
        }

        let err_str = "Timeout".to_string();
        debug!("{}", err_str);
        let _ = message.delete(&ctx).await;
        return Ok(());
    }
}

async fn set_fn(
    ctx: PContext<'_>,
    announcement_name: &String,
    discord_user: &User,
    discord_name: &String,
) -> Result<(), PError> {
    let path_string = format!("/config/index/{}/{}.flac", discord_name, announcement_name);
    let path = Path::new(&path_string);

    if !path.exists() {
        let why = "File doesn't exist.";
        let err_str = format!("Please choose a valid announcement. Path={}", &path_string);
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

    let user_id = discord_user.id.get();
    let insert_res = db.execute(
        "INSERT OR REPLACE INTO names (name, user_id, active_file)
            VALUES (?1, ?2, ?3)",
        params![discord_name, user_id as i64, announcement_name],
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

            let announcement: CreateSelectMenuOption = CreateSelectMenuOption::new(announcement_name.clone(), announcement_name)
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
    prev: bool,
    next: bool,
    options: Vec<CreateSelectMenuOption>,
) -> Vec<CreateActionRow> {
    let mut action_row = vec![
        CreateActionRow::SelectMenu(
            CreateSelectMenu::new(ANNOUNCEMENT_SELECTOR_DROPDOWN, CreateSelectMenuKind::String {
                options
            })
            .min_values(1)
            .max_values(1)
        )
    ];

    if prev || next {
        create_navigation_buttons(prev, next).append(&mut action_row)
    }

    return action_row;
}
