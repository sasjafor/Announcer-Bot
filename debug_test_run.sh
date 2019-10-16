#!/bin/bash
cargo build
DISCORD_APP_AUTH_TOKEN=MzQ3MzgxNTQyOTQ0NzY4MDEx.DuGkkQ.ZXCOL-djADimMNgfONnLhRpqjuM RUST_LOG=debug ./target/debug/announcer_bot
