# syntax=docker/dockerfile:experimental
FROM rust:1.43-buster@sha256:afeb25419be9f7b69481bd5ad37f107a87fca1bb205a5b694a9f0c9136b5788f as build
VOLUME ["/usr/local/cargo", "/stovbot/target"]
WORKDIR /app
RUN USER=root cargo init

RUN rustup default nightly

COPY . ./
RUN \
 --mount=type=cache,target=/usr/local/cargo/git \
 --mount=type=cache,target=/usr/local/cargo/registry \
 --mount=type=cache,target=/app/target \
 cargo build --release && \
 strip target/release/stovbot && \
 strip target/release/script_engine && \
 cp /app/target/release/stovbot /stovbot && \
 cp /app/target/release/script_engine /script_engine

FROM debian:buster@sha256:f19be6b8095d6ea46f5345e2651eec4e5ee9e84fc83f3bc3b73587197853dc9e
WORKDIR /app
ENTRYPOINT ["/stovbot"]
RUN apt-get update && \
    apt-get install -y \
        sqlite3 openssl ca-certificates \
        libfontconfig libxcb1 && \
    rm -rf /var/lib/apt/lists/*
COPY --from=build /script_engine /script_engine
COPY --from=build /stovbot /stovbot
