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
    @printf "  idp [CONFIG]   Run the standalone IdP server\n"
    @printf "  management [CONFIG] Run the standalone Management server\n"
    @printf "  device [CONFIG] Run the standalone Device server\n"
    @printf "  storage [CONFIG] Run the standalone Storage server\n"
    @printf "  unified [CONFIG] Run the unified server\n"
    @printf "  idp-portless [CONFIG]   Run the standalone IdP server via portless\n"
    @printf "  management-portless [CONFIG] Run the standalone Management server via portless\n"
    @printf "  device-portless [CONFIG] Run the standalone Device server via portless\n"
    @printf "  storage-portless [CONFIG] Run the standalone Storage server via portless\n"
    @printf "  unified-portless [CONFIG] Run the unified server via portless\n"
    @printf "  portless-servers          Show commands to run all standalone servers via portless\n"

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
    cargo run -p idp-server --features cli --bin idp-server -- --config "{{ config }}" {{ args }}

management config="config/management/config.yaml" *args:
    cargo run -p management-server --features cli --bin management-server -- --config "{{ config }}" {{ args }}

device config="config/device/config.yaml" *args:
    cargo run -p device-server --features cli --bin device-server -- --config "{{ config }}" {{ args }}

storage config="config/storage/config.yaml" *args:
    cargo run -p storage-server --features cli --bin storage-server -- --config "{{ config }}" {{ args }}

unified config="config/unified/config.yaml" *args:
    cargo run -p unified-server --features cli --bin unified-server -- --config "{{ config }}" {{ args }}

openapi-export:
    cargo run -p unified-server --example export-openapi -- openapi

openapi-generate:
    pnpm openapi

# Portless server recipes - run servers via portless proxy (auto HTTPS, stable .localhost URLs)
# These require portless to be installed (npm install -g portless or npx portless)
# Note: Run each in a separate terminal. portless manages the proxy and server processes.

idp-portless config="config/idp/config.yaml" *args:
    cargo build -p idp-server --features cli --bin idp-server
    npx portless idp ./target/debug/idp-server --config "{{ config }}" {{ args }}

management-portless config="config/management/config.yaml" *args:
    cargo build -p management-server --features cli --bin management-server
    npx portless management ./target/debug/management-server --config "{{ config }}" {{ args }}

device-portless config="config/device/config.yaml" *args:
    cargo build -p device-server --features cli --bin device-server
    npx portless device ./target/debug/device-server --config "{{ config }}" {{ args }}

storage-portless config="config/storage/config.yaml" *args:
    cargo build -p storage-server --features cli --bin storage-server
    npx portless storage ./target/debug/storage-server --config "{{ config }}" {{ args }}

unified-portless config="config/unified/config.yaml" *args:
    cargo build -p unified-server --features cli --bin unified-server
    npx portless unified ./target/debug/unified-server --config "{{ config }}" {{ args }}

# Run all standalone servers via portless (run each in separate terminal)
portless-servers:
    @echo "Run each in a separate terminal:"
    @echo "  just idp-portless"
    @echo "  just management-portless"
    @echo "  just device-portless"
    @echo "  just storage-portless"
