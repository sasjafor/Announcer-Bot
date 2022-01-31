

# Announcer Bot
[![Build Status](https://img.shields.io/github/workflow/status/sasjafor/Announcer-Bot/docker-image-ci?style=flat-square)](https://github.com/sasjafor/Announcer-Bot/actions/workflows/docker-image-ci.yml) [![License](https://img.shields.io/badge/license-GPL-lightgrey.svg?style=flat-square)](https://opensource.org/licenses/gpl-license) [![Discord Server](https://img.shields.io/discord/518113399448666113.svg?style=flat-square&colorB=7289DA)](https://discord.gg/qPxJfWw)

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
