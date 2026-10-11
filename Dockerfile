FROM rust:1.99.0-alpine3.24 AS build

WORKDIR /app
RUN apk add --no-cache build-base cmake pkgconf
ENV OPUS_BUNDLED=1

COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN RUSTFLAGS='-C link-arg=-s' cargo build --locked --release

FROM alpine:3.24
ENV RUST_LOG=warn

RUN apk add --no-cache ca-certificates espeak ffmpeg
COPY --from=build /app/target/release/announcer_bot /usr/local/bin/announcer_bot
ADD https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp /usr/local/bin/yt-dlp
RUN chmod a+rx /usr/local/bin/yt-dlp

VOLUME ["/config"]
CMD ["/usr/local/bin/announcer_bot"]
