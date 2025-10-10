FROM rust:1.90-trixie as builder

# Create empty shell project
RUN USER=root cargo new --bin announcer_bot

WORKDIR /announcer_bot

# Copy manifest
COPY ./Cargo.toml ./Cargo.toml

# Install cmake
RUN apt-get update && \
    apt-get install -y --no-install-recommends cmake libopus-dev

# Build dependencies
RUN RUSTFLAGS='-C link-arg=-s' cargo build --release

RUN rm src/*.rs

ADD . ./

# Build for release
RUN rm ./target/release/deps/announcer_bot*
RUN RUSTFLAGS='-C link-arg=-s' cargo build --release

FROM debian:trixie-slim

# Set log level
ENV RUST_LOG announcer_bot=info

# Setup apt, install package dependencies and create /config
RUN apt-get update && \
    apt-get install -y --no-install-recommends  ca-certificates \
                                                espeak \
                                                ffmpeg \
                                                lame \
                                                libopus0 \
                                                libopus-dev \
                                                libsqlite3-dev \
                                                python-is-python3 \
                                                vorbis-tools \
                                                && \
    mkdir /config

# Copy run script
COPY src/run /bin

# Copy executable
COPY --from=builder /announcer_bot/target/release/announcer_bot /bin

WORKDIR /

# Install yt-dlp
ADD https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp /usr/local/bin/
RUN chmod a+rx /usr/local/bin/yt-dlp

# Set run command
VOLUME /config
CMD ["run"]
