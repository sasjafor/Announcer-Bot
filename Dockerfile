FROM node:boron

# Copy source
COPY usr/src/app /usr/src/app

# Copy files to /config and install dependencies
RUN mkdir /config && \
    cd /config && \
    cp /usr/src/app/* . && \
    npm install discord.io winston --save

EXPOSE 8080
VOLUME /config
CMD [ "npm", "start" ]
