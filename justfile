set shell := ["bash", "-cu"]

# Default task when invoking `just` with no arguments.
default: help

help:
    @printf "Available recipes:\n"
    @printf "  build          Build all workspace crates\n"
    @printf "  build-release  Build all workspace crates in release mode\n"
    @printf "  check          Check all workspace crates\n"
    @printf "  test           Run workspace tests\n"
    @printf "  clippy         Run clippy for all targets and workspace crates\n"
    @printf "  clippy-fix     Run clippy with --fix for all targets and workspace crates\n"
    @printf "  crap           Run CRAP\n"
    @printf "  crap-summary   Run CRAP and get a summary\n"
    @printf "  fmt            Format all workspace crates\n"
    @printf "  fmt-check      Check formatting for all workspace crates\n"
    @printf "  clean          Remove build artifacts\n"
    @printf "  doc            Build workspace documentation\n"
    @printf "  boundary-check Check server dependency boundaries\n"
    @printf "  idp [CONFIG]   Run the IdP server via portless (cargo run)\n"
    @printf "  management [CONFIG] Run the Management server via portless (cargo run)\n"
    @printf "  device [CONFIG] Run the Device server via portless (cargo run)\n"
    @printf "  storage [CONFIG] Run the Storage server via portless (cargo run)\n"
    @printf "  unified [CONFIG] Run the unified server via portless (cargo run)\n"

build:
    cargo build --workspace

build-release:
    cargo build --workspace --release

check:
    cargo check --workspace

sdk-build:
    pnpm sdk:build

test: sdk-build
    cargo hack test --feature-powerset --workspace --all-targets

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

clippy-fix:
    cargo clippy --workspace --all-targets --fix --allow-dirty --broken-code -- -D warnings

crap *args: sdk-build
    RUST_MIN_STACK=67108864 cargo llvm-cov --workspace --lcov --output-path /tmp/lcov.info && cargo crap --workspace --lcov /tmp/lcov.info  {{ args }}

crap-summary:
    just crap --summary

cov: sdk-build
    RUST_MIN_STACK=67108864 cargo llvm-cov --workspace --open

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

clean:
    cargo clean

doc:
    cargo doc --workspace --no-deps

boundary-check:
    python3 scripts/check_server_dependencies.py

idp config="config/idp/config.yaml" *args:
    npx portless run --name idp cargo run -p idp-server --features cli --bin idp-server -- --config "{{ config }}" {{ args }}

management config="config/management/config.yaml" *args:
    npx portless run --name management cargo run -p management-server --features cli --bin management-server -- --config "{{ config }}" {{ args }}

device config="config/device/config.yaml" *args:
    npx portless run --name device cargo run -p device-server --features cli --bin device-server -- --config "{{ config }}" {{ args }}

storage config="config/storage/config.yaml" *args:
    npx portless run --name storage cargo run -p storage-server --features cli --bin storage-server -- --config "{{ config }}" {{ args }}

unified config="config/unified/config.yaml" *args:
    npx portless run --name unified cargo run -p unified-server --features cli --bin unified-server -- --config "{{ config }}" {{ args }}

openapi-export:
    cargo run -p unified-server --example export-openapi -- openapi

openapi-generate:
    pnpm openapi
