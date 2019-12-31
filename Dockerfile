FROM rust:1.40-buster@sha256:b9206ab8057d1e851e6286802eebeca5dadc78c73788cf25c5da4be7ac8363fa as build
WORKDIR /stovbot
RUN USER=root cargo init

COPY Cargo.lock Cargo.toml ./
RUN cargo build --release --no-default-features

COPY src/ src/
RUN rm -f target/release/deps/stovbot* && cargo build --release --no-default-features

FROM debian:buster@sha256:f19be6b8095d6ea46f5345e2651eec4e5ee9e84fc83f3bc3b73587197853dc9e
WORKDIR /app
ENTRYPOINT ["/stovbot"]
RUN apt-get update && apt-get install -y sqlite3 && rm -rf /var/lib/apt/lists/*
COPY --from=build /stovbot/target/release/stovbot /stovbot