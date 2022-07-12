use std::{ffi::OsStr, fs, path::Path};

use serenity::{
    builder::{CreateApplicationCommand, CreateComponents, CreateEmbed},
    client::Context,
    model::{
        interactions::{
            application_command::ApplicationCommandOptionType, message_component::ButtonStyle,
        },
        prelude::*,
    },
    utils::Colour,
};

use tracing::error;

use rusqlite::{params, Connection, OptionalExtension};

const EMBED_DESCRIPTION_MAX_LENGTH: usize = 4096;
const ELEMENTS_PER_PAGE: usize = 10;

pub fn create_list_command(
    command: &mut CreateApplicationCommand,
) -> &mut CreateApplicationCommand {
    return command
        .name("list")
        .description("List all available announcements for a name.")
        .create_option(|option| {
            option
                .name("name")
                .description("The name for which to list announcements.")
                .kind(ApplicationCommandOptionType::String)
        })
        .create_option(|option| {
            option
            .name("index")
            .description("The page index.")
            .kind(ApplicationCommandOptionType::Integer)
            .min_int_value(1)
        });
}

pub async fn list(
    _ctx: &Context,
    user: &User,
    name: &String,
    index: usize,
) -> (String, Option<CreateEmbed>, Option<CreateComponents>) {
    let path_string = format!("/config/index/{}", &name);
    let path = Path::new(&path_string);

    let db_path = Path::new("/config/database/db.sqlite");

    let db = match Connection::open(&db_path) {
        Ok(db) => db,
        Err(err) => {
            let err_str = "Failed to open database:".to_string();
            error!("{} {}", err_str, err);
            return (err_str, None, None);
        }
    };

    let filename = match db
        .query_row::<String, _, _>(
            "SELECT active_file FROM names WHERE name=?1",
            params![&name],
            |row| row.get(0),
        )
        .optional()
    {
        Ok(filename) => filename,
        Err(err) => {
            let err_str = format!("Failed to query active file for {}", name);
            error!("{}, Error Code {}", err_str, err);
            return (err_str, None, None);
        }
    };

    if path.is_dir() {
        let mut dir_iterator = match fs::read_dir(path) {
            Ok(dir_iterator) => dir_iterator,
            Err(err) => {
                let err_str = format!("Failed to read directory {}", path_string);
                error!("{}, Error: {}", err_str, err);
                return (err_str, None, None);
            }
        };
        // count elements and then recreate iterator
        let entry_count = dir_iterator.count();
        dir_iterator = match fs::read_dir(path) {
            Ok(dir_iterator) => dir_iterator,
            Err(err) => {
                let err_str = format!("Failed to read directory {}", path_string);
                error!("{}, Error: {}", err_str, err);
                return (err_str, None, None);
            }
        };
        let last_page_index = (entry_count as f64 / ELEMENTS_PER_PAGE as f64).ceil() as usize;
        if index > last_page_index {
            return ("Index too large, provide a valid index.".to_string(), None, None);
        }

        let mut active_filename = None;
        let active_filename_str;
        if filename.is_some() {
            active_filename_str = filename.unwrap();
            active_filename = Some(OsStr::new(&active_filename_str));
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
                    error!("{}, Error: {}", err_str, err);
                    return (err_str, None, None);
                }
            };
            let mut line_str = "".to_owned();
            if !entry_value.path().is_dir() {
                if active_filename.is_some()
                    && entry_value.path().file_stem().unwrap() == active_filename.unwrap()
                {
                    line_str = format!(
                        "• `{}` <=={:=>30}",
                        entry_value.path().file_stem().unwrap().to_str().unwrap(),
                        format!(" {}", &user.mention())
                    );
                } else {
                    line_str = format!(
                        "• `{}`",
                        entry_value.path().file_stem().unwrap().to_str().unwrap()
                    );
                }
            }

            let line_len = line_str.chars().count();
            if count - start_pos > ELEMENTS_PER_PAGE {
                break;
            }
            count += 1;
            if msg_len + line_len > EMBED_DESCRIPTION_MAX_LENGTH {
                return ("Embed too long.".to_string(), None, None);
            }

            msg_str.push_str("\n");
            msg_str.push_str(&line_str);
            msg_len += line_len + 1;
        }

        let mut embed = CreateEmbed::default();
        embed
            .title(format!("Announcements for \"{}\"", &name))
            .description(msg_str)
            .colour(Colour::from_rgb(128, 128, 128))
            .footer(|footer| {
                footer.text(format!("Page {}/{}", index, last_page_index))
            });

        let mut components = CreateComponents::default();
        components.create_action_row(|row| {
            row.create_button(|button| {
                button
                    .custom_id("Prev Button")
                    .label("Previous")
                    .style(ButtonStyle::Secondary);

                if index == 1 {
                    button.disabled(true);
                }
                button
            })
            .create_button(|button| {
                button
                    .custom_id("Next Button")
                    .label("Next")
                    .style(ButtonStyle::Secondary);

                if index == last_page_index {
                    button.disabled(true);
                }
                button
            })
        });

        ("".to_string(), Some(embed), Some(components))
    } else {
        ("This name doesn't exist".to_string(), None, None)
    }
}
