# Announcer Bot
[![npm link](https://nodei.co/npm/announcer-bot.png?downloads=true&downloadRank=true)](https://www.npmjs.com/package/announcer-bot)

[![Build Status](https://travis-ci.org/sasjafor/Announcer-Bot.svg)](https://travis-ci.org/sasjafor/Announcer-Bot) [![Total Downloads](https://img.shields.io/npm/dt/announcer-bot.svg)](https://www.npmjs.com/package/announcer-bot) [![Latest Stable Version](https://img.shields.io/npm/v/announcer-bot.svg)](https://www.npmjs.com/package/announcer-bot) [![Dependencies](https://david-dm.org/sasjafor/Announcer-Bot/status.svg)](https://david-dm.org/sasjafor/Announcer-Bot) [![License](https://img.shields.io/badge/license-GPL-lightgrey.svg)](https://opensource.org/licenses/gpl-license) [![Discord Server](https://discordapp.com/api/guilds/518113399448666113/embed.png)](https://discord.gg/qPxJfWw) [![Greenkeeper badge](https://badges.greenkeeper.io/sasjafor/Announcer-Bot.svg)](https://greenkeeper.io/)

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
