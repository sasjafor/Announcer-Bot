FROM rust:1.33

# Copy run script
COPY src/run /bin

# Install rust toolchain
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y

# Setup apt, install package dependencies and create /config
RUN echo "deb http://ftp.debian.org/debian jessie-backports main" >> /etc/apt/sources.list && \
    apt-get update && \
    apt-get install -y --no-install-recommends  libespeak-dev \
                                                lame \
                                                libopus0 \
                                                libssl-dev \
                                                vorbis-tools \
                                                && \
    mkdir /config

WORKDIR /usr/src

RUN USER=root cargo new app

COPY Cargo.toml /usr/src/app

WORKDIR /usr/src/app

RUN cargo build --release

COPY src /usr/src/app/src

RUN cargo build --release && \
    mv target/release/announcer_bot /bin && \
    rm -rf /usr/src/app

WORKDIR /

# EXPOSE 8080
VOLUME /config
CMD ["run"]
