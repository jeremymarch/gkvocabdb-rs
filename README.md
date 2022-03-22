### gkvocabdb

install rust:
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

build:
    cargo run

set environment variable for sqlite db: 
    export GKVOCABDB_DB_PATH=sqlite://gkvocabnew.sqlite?mode=rwc

Open web browser at http://0.0.0.0:8088
