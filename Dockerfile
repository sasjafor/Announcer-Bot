FROM node

# Copy source
COPY root/ /

# Make /config and install dependencies
RUN echo "deb http://ftp.debian.org/debian jessie-backports main" >> /etc/apt/sources.list && \
    mkdir /config && \
    cd /usr/src/app && \
    npm install --save-prod

EXPOSE 8080
VOLUME /config
CMD ["/usr/src/run.sh"]
