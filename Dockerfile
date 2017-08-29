FROM node

# Copy source
COPY root/ /

# Make /config and install dependencies
RUN deb http://ftp.debian.org/debian jessie-backports main && \
    apt-get update && \
    apt-get install -y --no-install-recommends ffmpeg && \
    mkdir /config && \
    cd /usr/src/app && \
    npm install discord.io winston --save

EXPOSE 8080
VOLUME /config
CMD ["/usr/src/run.sh"]
