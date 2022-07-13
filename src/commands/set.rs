use std::{path::Path};

use serenity::{
    builder::{CreateApplicationCommand, CreateComponents, CreateEmbed},
    client::Context,
    model::{interactions::application_command::ApplicationCommandOptionType, prelude::*},
    utils::Colour,
};

use tracing::{debug, error};
use rusqlite::{params, Connection};

pub fn create_set_command(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    return command
        .name("set")
        .description("Set the active announcement for the current nickname.")
        .create_option(|option| {
            option
                .name("name")
                .description("The name for which to list announcements.")
                .kind(ApplicationCommandOptionType::String)
        })
        .create_option(|option| {
            option
                .name("index")
                .description("The user for which to set the active announcement.")
                .kind(ApplicationCommandOptionType::User)
        });
}

pub async fn set(
    _ctx: &Context,
    name: &String,
    announcement_name: &String,
    user: User,
) -> (String, Option<CreateEmbed>, Option<CreateComponents>) {
    let path_string = format!("/config/index/{}/{}.wav", &name, &announcement_name);
    let path = Path::new(&path_string);

    if !path.exists() {
        let err_str = "Please choose a valid announcement".to_string();
        debug!("{}: {}", err_str, "File doesn't exist.");
        return (err_str, None, None);
    }

    let db_path = Path::new("/config/database/db.sqlite");

    let db = match Connection::open(&db_path) {
        Ok(db) => db,
        Err(why) => {
            let err_str = "Failed to open database".to_string();
            error!("{}: {}", err_str, why);
            return (err_str, None, None);
        }
    };

    let user_id = user.id.as_u64();
    let _ = match db.execute(
        "INSERT OR REPLACE INTO names (name, user_id, active_file)
            VALUES (?1, ?2, ?3)",
        params![&name, *user_id as i64, &announcement_name],
    ) {
        Ok(_) => (),
        Err(why) => {
            let err_str = "Failed to insert new name".to_string();
            error!("{}: {}", err_str, why);
            return (err_str, None, None);
        }
    };

    let mut embed = CreateEmbed::default();
    embed.title(format!("Set announcement"));
    embed.description(format!("`{}` [{}]", &announcement_name, &user.mention()));
    embed.colour(Colour::from_rgb(128, 128, 128));

    return ("".to_string(), Some(embed), None);
}
