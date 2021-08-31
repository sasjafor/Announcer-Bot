use serenity::{
    client::{
        Context,
    },
    model::{
        id::{
            ChannelId,
        }, 
        prelude::*,
    },
};

/// Checks if user can connect to a voice channel
pub async fn can_connect(ctx: &Context, channel_id: ChannelId) -> bool {
    let channel = match ctx.cache.guild_channel(channel_id).await {
        Some(channel) => channel,
        None => return false,
    };

    let current_user_id = ctx.cache.current_user().await.id;
    if let Ok(permissions) = channel.permissions_for_user(&ctx.cache, current_user_id).await {
        if permissions.contains(Permissions::CONNECT) {
            return true;
        }
    }

    return false;
}