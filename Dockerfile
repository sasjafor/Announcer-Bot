FROM rust:1.43

# Install rust toolchain
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y

# Setup apt, install package dependencies and create /config
RUN apt-get update && \
    apt-get install -y --no-install-recommends  espeak \
                                                ffmpeg \
                                                lame \
                                                libopus0 \
                                                libsqlite3-dev \
                                                libssl-dev \
                                                python3 \
                                                python3-pip \
                                                python3-setuptools \
                                                vorbis-tools \
                                                && \
    mkdir /config

# Install ffmpeg-normalize
RUN pip3 install ffmpeg-normalize

# Create empty shell project
RUN USER=root cargo new --bin announcer_bot

WORKDIR /announcer_bot

# Set log level
ENV RUST_LOG warn

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

# Install youtube-dl
ADD https://yt-dl.org/downloads/latest/youtube-dl /usr/local/bin/
RUN chmod a+rx /usr/local/bin/youtube-dl

# EXPOSE 8080
VOLUME /config
CMD ["run"]
