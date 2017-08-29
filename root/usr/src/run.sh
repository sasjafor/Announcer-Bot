#!/bin/bash
cp -n /usr/src/app/* /config/
AUTH_TOKEN=${DISCORD_APP_AUTH_TOKEN}
node /config/bot.js
