use std::{fs, path::Path, time::Duration};
use tracing::debug;

use poise::{futures_util::StreamExt, CreateReply};
use serenity::{
    all::{
        CreateEmbed, 
        CreateEmbedFooter, 
        CreateInteractionResponse, 
        CreateInteractionResponseMessage},
    model::colour::Colour,
};

use crate::{
    util::{
        component_ids::{NAMES_LIST_NEXT_BUTTON, NAMES_LIST_PREV_BUTTON}, consts::EMBED_DESCRIPTION_MAX_LENGTH, messages::create_navigation_buttons, util::{send_debug, send_error, send_warning}
    },
    PContext, PError,
};

const TIMEOUT_DURATION: Duration = Duration::from_secs(300);
const ELEMENTS_PER_PAGE: usize = 10;

#[doc = "List all names that have available announcements."]
#[poise::command(
    category = "Main Commands",
    guild_only,
    prefix_command,
    slash_command,
    required_bot_permissions = "SEND_MESSAGES"
)]
pub async fn names(
    ctx: PContext<'_>,
    #[description = "The page index."]
    #[min = 1]
    index: Option<usize>,
) -> Result<(), PError> {
    let mut index = index.unwrap_or(1);

    let path_string = format!("/config/index/");
    let path = Path::new(&path_string);

    if path.is_dir() {
        let (content, last_page_index) = match create_list(ctx, path, &path_string, index).await {
            Ok(res) => res,
            Err(why) => return Err(why),
        };

        let mut components = vec![];
        if last_page_index > 1 {
            components = create_navigation_buttons(index != 1, index < last_page_index);
        }

        let reply = CreateReply::default()
            .embed(CreateEmbed::new()
                .title(format!("List of all names that have announcements"))
                .description(content)
                .colour(Colour::from_rgb(128, 128, 128))
                .footer(CreateEmbedFooter::new(format!("Page {}/{}", index, last_page_index)))
            )
            .components(components);

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
                NAMES_LIST_PREV_BUTTON => {
                    index -= 1;
                }
                NAMES_LIST_NEXT_BUTTON => {
                    index += 1;
                }
                _ => {
                    let why = &interaction.data.custom_id;
                    let err_str = "Unknown component interaction".to_string();
                    return send_warning(ctx, err_str, why.to_string()).await;
                }
            }
            let (content, last_page_index) = match create_list(ctx, path, &path_string, index).await {
                Ok(res) => res,
                Err(why) => return Err(why),
            };

            let mut components = vec![];
            if last_page_index > 1 {
                components = create_navigation_buttons(index != 1, index < last_page_index);
            }

            let interaction_response = CreateInteractionResponseMessage::default()
                .embed(CreateEmbed::new()
                    .title(format!("List of all names that have announcements"))
                    .description(content)
                    .colour(Colour::from_rgb(128, 128, 128))
                    .footer(CreateEmbedFooter::new(format!("Page {}/{}", index, last_page_index)))
                )
                .components(components);

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
    } else {
        let why = path_string;
        let err_str = "Index directory doesn't exist".to_string();
        return send_warning(ctx, err_str, why.to_string()).await;
    }
}

async fn create_list(
    ctx: PContext<'_>,
    path: &Path,
    path_string: &String,
    index: usize,
) -> Result<(String, usize), PError> {
    let mut dir_iterator = match fs::read_dir(path) {
        Ok(dir_iterator) => dir_iterator,
        Err(why) => {
            let err_str = format!("Failed to read directory {}", path_string);
            return match send_error(ctx, err_str, why.to_string()).await {
                Ok(_) => Err(Into::into(why)),
                Err(why) => Err(why),
            };
        }
    };
    // count elements and then recreate iterator
    let entry_count = dir_iterator.count();
    dir_iterator = match fs::read_dir(path) {
        Ok(dir_iterator) => dir_iterator,
        Err(why) => {
            let err_str = format!("Failed to read directory {}", path_string);
            return match send_error(ctx, err_str, why.to_string()).await {
                Ok(_) => Err(Into::into(why)),
                Err(why) => Err(why),
            };
        }
    };
    let last_page_index = (entry_count as f64 / ELEMENTS_PER_PAGE as f64).ceil() as usize;
    if index > last_page_index {
        let why = index;
        let err_str = "Index too large, provide a valid index.".to_string();
        return match send_debug(ctx, err_str, why.to_string()).await {
            Ok(_) => Err(Into::into(serenity::Error::Other("Err"))),
            Err(why) => Err(why),
        };
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
            Err(why) => {
                let err_str = "Failed to get next entry".to_string();
                return match send_error(ctx, err_str, why.to_string()).await {
                    Ok(_) => Err(Into::into(why)),
                    Err(why) => Err(why),
                };
            }
        };
        let line_str;
        if entry_value.path().is_dir() {
            line_str = format!("• `{}`", entry_value.path().file_stem().unwrap().to_str().unwrap());
        } else {
            continue;
        }

        let line_len = line_str.chars().count();
        count += 1;
        if count - start_pos > ELEMENTS_PER_PAGE {
            break;
        }
        if msg_len + line_len > EMBED_DESCRIPTION_MAX_LENGTH {
            let why = msg_len + line_len;
            let err_str = "Embed too long".to_string();
            return match send_warning(ctx, err_str, why.to_string()).await {
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
