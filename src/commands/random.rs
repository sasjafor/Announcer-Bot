use std::{
    path::Path, 
};

use serenity::{
    client::{
        Context,
    },
    // framework::{
    //     standard::{
    //         macros::{
    //             command,
    //         }, 
    //         Args,
    //         CommandResult,
    //     },
    // },
    model::{
        prelude::*,
    },
    utils::Colour,
};

use tracing::{error};

use rusqlite::{
    Connection,
    params,
};

use crate::lib::msg::check_msg;

// #[command]
// #[aliases("rand")]
// #[description("Toggle random mode")]
// #[usage("")]
// #[example("")]
// #[num_args(0)]
// #[help_available]
// pub async fn random(ctx: &Context, message: &Message, _args: Args) -> CommandResult {
//     let option_nick = &message.author_nick(&ctx).await;
//     let name = match option_nick {
//         Some(nick) => nick,
//         None => &message.author.name
//     };

//     let db_path = Path::new("/config/database/db.sqlite");

//     let db = match Connection::open(&db_path) {
//         Ok(db) => db,
//         Err(err) => {
//             error!("Failed to open database: {}", err);
//             return Ok(());
//         }
//     };

//     let user_id = message.author.id.as_u64();
//     let _ = match db.execute(
//         "UPDATE names SET random = ((random | 1) - (random & 1)) WHERE name=?1 AND user_id=?2",
//         params![&name, *user_id as i64]) {
//             Ok(_) => (),
//             Err(err) => {
//                 error!("Failed to random for name {}, Error Code {}", &name, err);
//                 return Ok(());
//             }
//     };

//     let msg_res = message.channel_id.send_message(&ctx, |m| {
//         m.embed(|e| {
//             e.title(format!("Toggled random"));
//             e.description(format!("**{}** [{}]", &name, &message.author.mention()));
//             e.colour(Colour::from_rgb(128,128,128));

//             e
//         });

//         m
//     });
//     check_msg(msg_res.await);
//     return Ok(());
// }