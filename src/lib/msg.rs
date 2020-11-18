use serenity::{
    Result as SerenityResult,
    model::channel::Message,
};

use tracing::{error};

/// Checks that a message successfully sent; if not, then logs why to stdout.
pub fn check_msg(result: SerenityResult<Message>) {
    if let Err(why) = result {
        error!("Error sending message: {:?}", why);
    }
}