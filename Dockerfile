FROM node:boron

# Create app directory
WORKDIR /usr/src/app

# Install app dependencies
COPY usr/src/app/package.json .

RUN npm install discord.io winston --save

# Copy source
COPY usr/src/app .

EXPOSE 8080
CMD nodejs bot.js
