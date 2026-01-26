# syntax=docker/dockerfile:1.6
FROM rust:latest as build
VOLUME ["/usr/local/cargo", "/stovbot/target"]
WORKDIR /app
ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse
ENV CARGO_NET_GIT_FETCH_WITH_CLI=true

COPY Cargo.toml Cargo.lock ./
RUN \
 --mount=type=cache,target=/usr/local/cargo/git \
 --mount=type=cache,target=/usr/local/cargo/registry \
 cargo fetch --locked

COPY . ./
RUN \
 --mount=type=cache,target=/usr/local/cargo/git \
 --mount=type=cache,target=/usr/local/cargo/registry \
 --mount=type=cache,target=/app/target \
 cargo build --release --locked && \
 strip target/release/stovbot && \
 strip target/release/script_engine && \
 cp /app/target/release/stovbot /stovbot && \
 cp /app/target/release/script_engine /script_engine

FROM debian:bookworm-slim
WORKDIR /app
ENTRYPOINT ["/stovbot"]
RUN apt-get update && \
    apt-get install -y \
        sqlite3 openssl ca-certificates \
        libfontconfig1 libxcb1 && \
    rm -rf /var/lib/apt/lists/*
COPY --from=build /script_engine /script_engine
COPY --from=build /stovbot /stovbot
