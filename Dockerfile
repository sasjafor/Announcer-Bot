FROM node:boron

# Create app directory
WORKDIR /usr/src/app

# Install app dependencies
COPY usr/src/app/package.json .

RUN npm install discord.io winston --save

VOLUME /config

# Copy source
COPY usr/src/app/ /config/

EXPOSE 8080
CMD [ "npm", "start" ]
