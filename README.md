

# Announcer Bot
[![License](https://img.shields.io/badge/license-GPL-lightgrey.svg)](https://opensource.org/licenses/gpl-license) [![Discord Server](https://discordapp.com/api/guilds/518113399448666113/embed.png)](https://discord.gg/qPxJfWw)

A simple Discord bot that announces users joining a voice channel or unmuting themselves.



## Current Features
* Announce users joining a Discord voice channel, or users unmuting themselves while already in a channel
* Audio files are created using e-speak
* Use your own custom audio files to get announced

## Usage

To use the bot you need to provide an authorisation token for a Discord application with the `DISCORD_APP_AUTH_TOKEN` environment variable.

Custom audio files can be submitted in a text channel named "announcer-bot-submissions" using this syntax `!newfile [NAME]`.
If no name is provided, the name of the file is used, but any underscores are replaced with spaces.

The audio files are stored in /config/audio because the bot is intended to be used within a docker container.

[crates.io link]: https://crates.io/crates/serenity
[crates.io version]: https://img.shields.io/crates/v/serenity.svg?style=flat-square
[guild]: https://discord.gg/WBdGJCc
[guild-badge]: https://img.shields.io/discord/381880193251409931.svg?style=flat-square&colorB=7289DA
[rust 1.31.1+ badge]: https://img.shields.io/badge/rust-1.31.1+-93450a.svg?style=flat-square
[rust 1.31.1+ link]: https://blog.rust-lang.org/2018/12/20/Rust-1.31.1.html
