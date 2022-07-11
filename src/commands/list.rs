use std::{ffi::OsStr, fs, path::Path};

use serenity::{
    builder::{CreateApplicationCommand, CreateEmbed},
    client::Context,
    framework::standard::{macros::command, Args, CommandResult},
    model::{interactions::application_command::ApplicationCommandOptionType, prelude::*},
    utils::Colour,
};

use tracing::error;

use rusqlite::{params, Connection, OptionalExtension};

use crate::lib::msg::check_msg;

const EMBED_DESCRIPTION_MAX_LENGTH: usize = 4096;

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
                .required(true)
        });
}

#[command]
#[description("List all available announcements for a name")]
#[usage("<discordname>")]
#[example("")]
#[example("Yzarul")]
#[example("\"Mr Yzarul\"")]
#[min_args(0)]
#[max_args(1)]
#[help_available]
pub async fn list_old(ctx: &Context, message: &Message, args: Args) -> CommandResult {
    let arguments = args.raw_quoted().collect::<Vec<&str>>();

    let option_nick = &message.author_nick(&ctx).await;
    let name = match arguments.first() {
        Some(name) => *name,
        None => match option_nick {
            Some(nick) => nick,
            None => &message.author.name,
        },
    };

    return Ok(());
}

pub async fn list(ctx: &Context, user: &User, name: &String) -> (String, Option<CreateEmbed>) {
    let path_string = format!("/config/index/{}", &name);
    let path = Path::new(&path_string);

    let db_path = Path::new("/config/database/db.sqlite");

    let db = match Connection::open(&db_path) {
        Ok(db) => db,
        Err(err) => {
            let err_str = "Failed to open database:".to_string();
            error!("{} {}", err_str, err);
            return (err_str, None);
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
            return (err_str, None);
        }
    };

    if path.is_dir() {
        let dir_iterator = match fs::read_dir(path) {
            Ok(dir_iterator) => dir_iterator,
            Err(err) => {
                let err_str = format!("Failed to read directory {}", path_string);
                error!("{}, Error: {}", err_str, err);
                return (err_str, None);
            }
        };

        let mut active_filename = None;
        let active_filename_str;
        if filename.is_some() {
            active_filename_str = filename.unwrap();
            active_filename = Some(OsStr::new(&active_filename_str));
        }

        let mut msg_len = 0;
        let mut msg_str = "".to_string();
        for entry in dir_iterator {
            let entry_value = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    let err_str = "Failed to get next entry".to_string();
                    error!("{}, Error: {}", err_str, err);
                    return (err_str, None);
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
            // TODO: create pages if embed too long
            if msg_len + line_len > EMBED_DESCRIPTION_MAX_LENGTH {
                break;
            //     let msg_res = message.channel_id.send_message(&ctx, |m| {
            //         m.embed(|e| {
            //             e.title(format!("Announcements for \"{}\"", &name));
            //             e.description(msg_str);
            //             e.colour(Colour::from_rgb(128, 128, 128));

            //             e
            //         });

            //         m
            //     });
            //     check_msg(msg_res.await);

            //     msg_str = "".to_string();
            //     msg_len = 0;
            }

            msg_str.push_str("\n");
            msg_str.push_str(&line_str);
            msg_len += line_len + 1;
        }

        // let msg_res = message.channel_id.send_message(&ctx, |m| {
        //     m.embed();

        //     m
        // });
        // check_msg(msg_res.await);

        let mut embed = CreateEmbed::default();
        embed
            .title(format!("Announcements for \"{}\"", &name))
            .description(msg_str)
            .colour(Colour::from_rgb(128, 128, 128));

        ("".to_string(), Some(embed))
    } else {
        ("This name doesn't exist".to_string(), None)
        // check_msg(
        //     message
        //         .channel_id
        //         .say(&ctx, "This name doesn't exist")
        //         .await,
        // );
    }
}
