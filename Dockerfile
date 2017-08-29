FROM node:boron

# Copy source
COPY root/ /

# Make /config and install dependencies
RUN mkdir /config && \
    cd /usr/src/app && \
    npm install discord.io winston --save

EXPOSE 8080
VOLUME /config
CMD ["/usr/src/run.sh"]
