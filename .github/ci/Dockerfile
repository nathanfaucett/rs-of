FROM rust:1.100-trixie AS rust
FROM node:26-trixie

LABEL org.opencontainers.image.source="https://github.com/nathanfaucett/rs-of"

ENV CARGO_HOME=/usr/local/cargo \
    RUSTUP_HOME=/usr/local/rustup \
    PATH=/usr/local/cargo/bin:${PATH}

COPY --from=rust /usr/local/cargo /usr/local/cargo
COPY --from=rust /usr/local/rustup /usr/local/rustup

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
    libdbus-1-dev \
    libnss3-tools \
    libprotobuf-dev \
    openjdk-21-jdk-headless \
    openssl \
    pkg-config \
    protobuf-compiler \
    python3 \
    && rm -rf /var/lib/apt/lists/*

RUN rustup toolchain install stable --profile minimal \
    --component rustfmt \
    --component clippy \
    --component llvm-tools-preview \
    && rustup target add wasm32-unknown-unknown --toolchain 1.100

RUN cargo install just --locked \
    && cargo install cargo-llvm-cov --locked \
    && cargo install cargo-crap --locked \
    && cargo install cargo-hack --locked \
    && cargo install wasm-pack --version 0.13.1 --locked

RUN npm install --global pnpm@12.4.1

ARG PLAYWRIGHT_VERSION
RUN npx --yes "playwright@${PLAYWRIGHT_VERSION}" install-deps chromium \
    && rm -rf /var/lib/apt/lists/*
