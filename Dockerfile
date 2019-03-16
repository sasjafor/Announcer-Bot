FROM rust:1.33

# Install rust toolchain
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y

# Setup apt, install package dependencies and create /config
RUN echo "deb http://ftp.debian.org/debian jessie-backports main" >> /etc/apt/sources.list && \
    apt-get update && \
    apt-get install -y --no-install-recommends  espeak \
                                                ffmpeg \
                                                lame \
                                                libopus0 \
                                                libssl-dev \
                                                vorbis-tools \
                                                && \
    mkdir /config

# Create empty shell project
RUN USER=root cargo new --bin announcer_bot

WORKDIR /announcer_bot

# Copy manifest
COPY ./Cargo.toml ./Cargo.toml

# Build dependencies
RUN cargo build --release
RUN rm src/*.rs

# Copy run script
COPY src/run /bin

# Copy source tree
COPY ./src ./src

# Build for release
RUN rm ./target/release/deps/announcer_bot*
RUN cargo build --release

# Copy executable
RUN mv ./target/release/announcer_bot /bin && \
    rm -rf /announcer_bot

WORKDIR /

# EXPOSE 8080
VOLUME /config
CMD ["run"]
