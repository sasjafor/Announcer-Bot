use serenity::{
    client::Context,
    model::{
        id::ChannelId, prelude::*
    },
};

/// Checks if user can connect to a voice channel
pub fn can_connect(ctx: &Context, guild_id_opt: Option<GuildId>, channel_id_opt: Option<ChannelId>) -> bool {
    let guild_id = match guild_id_opt {
        Some(guild_id) => guild_id,
        None => return false,
    };

    let guild = match ctx.cache.guild(guild_id) {
        Some(guild) => guild,
        None => return false,
    };

    let channel_id = match channel_id_opt {
        Some(channel_id) => channel_id,
        None => return false,
    };

    let channel = match guild.channels.get(&channel_id) {
        Some(channel) => channel,
        None => return false,
    };

    let current_user_id = ctx.cache.current_user().id;

    let member = match guild.members.get(&current_user_id) {
        Some(member) => member,
        None => return false,
    };

    let permissions = guild.user_permissions_in(channel, member);
    if permissions.contains(Permissions::CONNECT) {
        return true;
    }

    return false;
}