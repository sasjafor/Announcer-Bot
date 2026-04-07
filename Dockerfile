FROM rust:1.94.1-alpine3.23 as builder

# Create empty shell project
RUN USER=root cargo new --bin announcer_bot

WORKDIR /announcer_bot

# Copy manifest
COPY ./Cargo.toml ./Cargo.toml

# Install dependencies
RUN apk update && \
    apk add autoconf \
            automake \
            build-base \
            cmake \
            file \
            g++ \
            libtool \
            openssl-dev \
            pkgconf

ENV OPUS_NO_PKG_CONFIG=1

# Build dependencies
RUN RUSTFLAGS='-C link-arg=-s' cargo build --release

RUN rm src/*.rs

ADD . ./

# Build for release
RUN rm ./target/release/deps/announcer_bot*
RUN RUSTFLAGS='-C link-arg=-s' cargo build --release

FROM rust:1.94.1-alpine3.23

# Set log level
ENV RUST_LOG announcer_bot=info

# Setup apt, install package dependencies and create /config
RUN apk update && \
    apk add --no-cache  ca-certificates \
                        espeak \
                        ffmpeg \
                        lame \
                        opus \
                        sqlite \
                        python3 \
                        vorbis-tools \
                        && \
    mkdir /config

# Copy run script
COPY src/run /usr/local/bin/

# Copy executable
COPY --from=builder /announcer_bot/target/release/announcer_bot /usr/local/bin/

WORKDIR /

# Install yt-dlp
ADD https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp /usr/local/bin/
RUN chmod a+rx /usr/local/bin/yt-dlp

# Set run command
VOLUME /config
CMD ["run"]
