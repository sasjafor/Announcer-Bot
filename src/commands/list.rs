use crate::{
    lib::{
        component_ids::{LIST_NEXT_BUTTON, LIST_PREV_BUTTON},
        util::send_error,
    },
    pContext, Error,
};
use std::{ffi::OsStr, fs, path::Path, time::Duration};

use poise::{futures_util::StreamExt, serenity_prelude::CreateComponents};
use serenity::{
    model::{interactions::message_component::ButtonStyle, prelude::*},
    utils::Colour,
};

use rusqlite::{params, Connection, OptionalExtension};
use tracing::{debug, error};

const TIMEOUT_DURATION: Duration = Duration::from_secs(300);
const EMBED_DESCRIPTION_MAX_LENGTH: usize = 4096;
const ELEMENTS_PER_PAGE: usize = 10;

#[doc = "List all available announcements for a name."]
#[poise::command(
    category = "Main Commands",
    guild_only,
    prefix_command,
    slash_command,
    required_bot_permissions = "SEND_MESSAGES"
)]
pub async fn list(
    ctx: pContext<'_>,
    #[description = "The name for which to list announcements."] name: Option<String>,
    #[description = "The page index."]
    #[min = 1]
    index: Option<usize>,
) -> Result<(), Error> {
    let name = match name {
        Some(name) => name,
        None => {
            if let Some(member) = ctx.author_member().await {
                member.display_name().into_owned()
            } else {
                ctx.author().name.clone()
            }
        }
    };
    let mut index = index.unwrap_or(1);

    let path_string = format!("/config/index/{}", &name);
    let path = Path::new(&path_string);

    let db_path = Path::new("/config/database/db.sqlite");

    let db = match Connection::open(&db_path) {
        Ok(db) => db,
        Err(err) => {
            let err_str = "Failed to open database:".to_string();
            error!("{}: {}", err_str, err);
            return send_error(ctx, err_str).await;
        }
    };

    let filename_res = db
        .query_row::<String, _, _>("SELECT active_file FROM names WHERE name=?1", params![&name], |row| {
            row.get(0)
        })
        .optional();

    if filename_res.is_err() {
        let err_str = format!("Failed to query active file for {}", name);
        error!("{}: {}", err_str, filename_res.err().unwrap());
        return send_error(ctx, err_str).await;
    }
    let filename = filename_res.unwrap();

    if path.is_dir() {
        let (content, last_page_index) = match create_list(ctx, path, &path_string, &filename, index).await {
            Ok(res) => res,
            Err(why) => return Err(why),
        };

        let message = ctx
            .send(|m| {
                m.embed(|e| {
                    e.title(format!("Announcements for \"{}\"", &name))
                        .description(content)
                        .colour(Colour::from_rgb(128, 128, 128))
                        .footer(|footer| footer.text(format!("Page {}/{}", index, last_page_index)))
                })
                .components(|c| {
                    create_components(c, index != 1, index < last_page_index);
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
                LIST_PREV_BUTTON => {
                    index -= 1;
                }
                LIST_NEXT_BUTTON => {
                    index += 1;
                }
                _ => {
                    let err_str = "Unknown component interaction".to_string();
                    debug!("{}", err_str);
                    return send_error(ctx, err_str).await;
                }
            }
            let (content, last_page_index) = match create_list(ctx, path, &path_string, &filename, index).await {
                Ok(res) => res,
                Err(why) => return Err(why),
            };
            if let Err(why) = interaction
                .create_interaction_response(&ctx.discord().http, |response| {
                    response
                        .kind(InteractionResponseType::UpdateMessage)
                        .interaction_response_data(|m| {
                            m.embed(|e| {
                                e.title(format!("Announcements for \"{}\"", &name))
                                    .description(content)
                                    .colour(Colour::from_rgb(128, 128, 128))
                                    .footer(|footer| footer.text(format!("Page {}/{}", index, last_page_index)))
                            })
                            .components(|c| {
                                create_components(c, index != 1, index < last_page_index);
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
    } else {
        let err_str = "This name doesn't exist".to_string();
        debug!("{}: {}", err_str, name);
        return send_error(ctx, err_str).await;
    }
}

async fn create_list(
    ctx: pContext<'_>,
    path: &Path,
    path_string: &String,
    filename: &Option<String>,
    index: usize,
) -> Result<(String, usize), Error> {
    let mut dir_iterator = match fs::read_dir(path) {
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
    // count elements and then recreate iterator
    let entry_count = dir_iterator.count();
    dir_iterator = match fs::read_dir(path) {
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
    let last_page_index = (entry_count as f64 / ELEMENTS_PER_PAGE as f64).ceil() as usize;
    if index > last_page_index {
        let err_str = "Index too large, provide a valid index.".to_string();
        debug!("{}: {}", err_str, index);
        return match send_error(ctx, err_str).await {
            Ok(_) => Err(Into::into(serenity::Error::Other("Err"))),
            Err(why) => Err(why),
        };
    }

    let mut active_filename = None;
    let active_filename_str;
    if filename.is_some() {
        active_filename_str = filename.as_ref().unwrap();
        active_filename = Some(OsStr::new(active_filename_str));
    }

    let mut msg_len = 0;
    let mut msg_str = "".to_string();
    let mut count = 0;
    let start_pos = (index - 1) * ELEMENTS_PER_PAGE;
    for entry in dir_iterator {
        if count < start_pos {
            count += 1;
            continue;
        }

        let entry_value = match entry {
            Ok(entry) => entry,
            Err(err) => {
                let err_str = "Failed to get next entry".to_string();
                error!("{}: {}", err_str, err);
                return match send_error(ctx, err_str).await {
                    Ok(_) => Err(Into::into(err)),
                    Err(why) => Err(why),
                };
            }
        };
        let mut line_str = "".to_owned();
        if !entry_value.path().is_dir() {
            if active_filename.is_some() && entry_value.path().file_stem().unwrap() == active_filename.unwrap() {
                line_str = format!(
                    "• `{}` <=={:=>30}",
                    entry_value.path().file_stem().unwrap().to_str().unwrap(),
                    format!(" {}", &ctx.author().mention())
                );
            } else {
                line_str = format!("• `{}`", entry_value.path().file_stem().unwrap().to_str().unwrap());
            }
        }

        let line_len = line_str.chars().count();
        count += 1;
        if count - start_pos > ELEMENTS_PER_PAGE {
            break;
        }
        if msg_len + line_len > EMBED_DESCRIPTION_MAX_LENGTH {
            let err_str = "Embed too long".to_string();
            debug!("{}: {}", err_str, msg_len + line_len);
            return match send_error(ctx, err_str).await {
                Ok(_) => Err(Into::into("Err")),
                Err(why) => Err(why),
            };
        }

        msg_str.push_str("\n");
        msg_str.push_str(&line_str);
        msg_len += line_len + 1;
    }

    return Ok((msg_str, last_page_index));
}

fn create_components(components: &mut CreateComponents, prev: bool, next: bool) -> () {
    components.create_action_row(|r| {
        r.create_button(|button| {
            button
                .custom_id(LIST_PREV_BUTTON)
                .label("Previous")
                .style(ButtonStyle::Secondary);

            if !prev {
                button.disabled(true);
            }
            button
        })
        .create_button(|button| {
            button
                .custom_id(LIST_NEXT_BUTTON)
                .label("Next")
                .style(ButtonStyle::Secondary);

            if !next {
                button.disabled(true);
            }
            button
        })
    });
}
