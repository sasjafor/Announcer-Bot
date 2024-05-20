use serenity::{
    client::Context,
    model::{
        id::ChannelId, 
        prelude::*,
    },
};

/// Checks if user can connect to a voice channel
pub fn can_connect(ctx: &Context, channel_id_opt: Option<ChannelId>) -> bool {
    let channel_id = match channel_id_opt {
        Some(channel_id) => channel_id,
        None => return false,
    };

    let channel = match ctx.cache.channel(channel_id) {
        Some(channel) => channel,
        None => return false,
    };

    let current_user_id = ctx.cache.current_user().id;
    if let Ok(permissions) = channel.permissions_for_user(&ctx.cache, current_user_id) {
        if permissions.contains(Permissions::CONNECT) {
            return true;
        }
    }

    return false;
}