FROM node:10

# Copy run script
COPY src/run.sh /usr/src/

# Copy package.json
COPY package.json /usr/src/app/

# Setup apt, install non-node dependencies and create /config
RUN echo "deb http://ftp.debian.org/debian jessie-backports main" >> /etc/apt/sources.list && \
    apt-get update && \
    apt-get install -y --no-install-recommends espeak lame vorbis-tools && \
    mkdir /config

# Install node dependencies
RUN cd /usr/src/app && \
    npm install --save-prod

# Copy bot script file
COPY src/bot.js /usr/src/app/

EXPOSE 8080
VOLUME /config
CMD ["/usr/src/run.sh"]
