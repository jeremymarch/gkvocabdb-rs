FROM rust:1.81.0 AS build
# ENV PKG_CONFIG_ALLOW_CROSS=1

WORKDIR /usr/src/gkvocabdb
COPY . .

# RUN cargo install --features "postgres" --path actix
RUN cargo build --features "postgres" --release

FROM gcr.io/distroless/cc-debian12

COPY --from=build /usr/src/gkvocabdb/target/release/main /usr/local/bin/gkvocabdb
COPY --from=build /usr/src/gkvocabdb/static/ /usr/local/bin/static/

# ENV GKVOCABDB_DB_PATH= set from outside
# ENV GKVOCABDB_KEY= set from outside

EXPOSE 8088

WORKDIR /usr/local/bin

CMD ["gkvocabdb"]
