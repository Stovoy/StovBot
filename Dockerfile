# syntax=docker/dockerfile:experimental
FROM rust:1.43-buster as build
VOLUME ["/usr/local/cargo", "/stovbot/target"]
WORKDIR /app
RUN USER=root cargo init

RUN rustup toolchain install nightly-2019-12-01 && \
    rustup default nightly-2019-12-01

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

FROM debian:buster
WORKDIR /app
ENTRYPOINT ["/stovbot"]
RUN sed -i 's|deb.debian.org/debian|archive.debian.org/debian|g; s|security.debian.org/debian-security|archive.debian.org/debian-security|g' /etc/apt/sources.list && \
    sed -i '/buster-updates/d' /etc/apt/sources.list && \
    echo 'Acquire::Check-Valid-Until \"false\";' > /etc/apt/apt.conf.d/99no-check-valid && \
    apt-get update && \
    apt-get install -y \
        sqlite3 openssl ca-certificates \
        libfontconfig libxcb1 && \
    rm -rf /var/lib/apt/lists/*
COPY --from=build /script_engine /script_engine
COPY --from=build /stovbot /stovbot
