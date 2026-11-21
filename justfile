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
    @printf "  sdk-build      Build TypeScript SDK packages\n"
    @printf "  build-wasm     Build all WASM crates for wasm32-unknown-unknown\n"
    @printf "  test-wasm      Run browser tests for WASM crates\n"
    @printf "  npm-lint       Lint npm workspace with biome\n"
    @printf "  browser-check  Bounded release validation (ofdb browser matrix)\n"
    @printf "  browser-accept Full browser acceptance tests\n"
    @printf "  browser-measure Filtered snapshot diagnostics\n"
    @printf "  browser-native Native protocol/storage/sync regressions\n"
    @printf "  browser-final  Independent final gates\n"
    @printf "  idp [CONFIG]   Run the IdP server via portless (cargo run)\n"
    @printf "  management [CONFIG] Run the Management server via portless (cargo run)\n"
    @printf "  device [CONFIG] Run the Device server via portless (cargo run)\n"
    @printf "  storage [CONFIG] Run the Storage server via portless (cargo run)\n"
    @printf "  unified        Run the local unified server via portless (cargo run)\n"
    @printf "  openapi-export Export OpenAPI spec\n"
    @printf "  openapi-generate Generate TypeScript clients\n"

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

# TypeScript/WASM
npm-install:
    pnpm install --frozen-lockfile

build-wasm:
    pnpm --filter @ofsdk/sql run build:wasm
    pnpm --filter @ofsdk/kv run build:wasm
    pnpm --filter @ofsdk/file-system run build:wasm
    pnpm --filter @ofsdk/net run build:wasm

test-wasm:
    wasm-pack test --headless --chrome packages/sql/pkg packages/kv/pkg packages/net/pkg packages/file-system/pkg
    wasm-pack test --headless --chrome db/wasm

npm-lint:
    pnpm -r run lint

# ofdb browser validation matrix (ported from ofdb justfile)
[positional-arguments]
browser-check output="target/browser-validation/check" timeout="600":
    #!/usr/bin/env bash
    source scripts/browser-evidence.sh
    begin "$1" "$2"
    target=wasm32-unknown-unknown
    for package in ofdb-kv ofdb-sql; do
        features=(indexeddb indexeddb,sync)
        if [[ $package == ofdb-sql ]]; then
            features=(indexeddb indexeddb,sql indexeddb,sync indexeddb,sql,sync indexeddb,sql,sync,native-stage)
        fi
        for feature in "${features[@]}"; do
            name="$package-${feature//,/-}"
            common=(-p "$package" --no-default-features --features "$feature")
            check "$name" cargo check --locked "${common[@]}" --lib --target "$target"
            GRAPH_POLICY=browser check "$name-graph" cargo tree --locked "${common[@]}" --target "$target" --edges normal --prefix none --format '{p}'
        done
        GRAPH_POLICY=remote check "$package-remote-graph" cargo tree --locked -p "$package" --no-default-features --features remote --edges normal --prefix none --format '{p}'
    done
    GRAPH_POLICY=browser check fixture-graph cargo tree --locked --manifest-path tests/browser/Cargo.toml --features sync,ofdb-sql/native-stage --target "$target" --edges normal --prefix none --format '{p}'
    for context in window worker; do
        feature=sync
        [[ $context != worker ]] || feature=worker,sync
        check "$context-clippy" cargo clippy --locked --manifest-path tests/browser/Cargo.toml --tests --target "$target" --features "$feature" -- -D warnings
    done
    check public-quickstarts bash scripts/browser-quickstarts.sh
    finish

[positional-arguments]
browser-accept browser="all" context="all" output="target/browser-validation/browser" timeout="600" chromedriver=env_var_or_default("CHROMEDRIVER", "") geckodriver=env_var_or_default("GECKODRIVER", ""):
    #!/usr/bin/env bash
    source scripts/browser-evidence.sh
    selection "$1" "$2"
    begin "$3" "$4"
    check wasm-pack wasm-pack --version
    for browser in "${browsers[@]}"; do
        driver=$5; option=--chromedriver
        if [[ $browser == firefox ]]; then driver=$6; option=--geckodriver; fi
        driver_args=()
        [[ -z $driver ]] || driver_args=("$option" "$driver")
        for context in "${contexts[@]}"; do
            feature=sync
            [[ $context != worker ]] || feature=worker,sync
            check "$browser-$context" wasm-pack test --headless "--$browser" "${driver_args[@]}" tests/browser --features "$feature" --lib
        done
    done
    finish

[positional-arguments]
browser-measure browser="all" context="all" output="target/browser-validation/measure" timeout="600" runner="wasm-bindgen-test-runner" chromedriver=env_var_or_default("CHROMEDRIVER", "") geckodriver=env_var_or_default("GECKODRIVER", ""):
    #!/usr/bin/env bash
    source scripts/browser-evidence.sh
    selection "$1" "$2"
    begin "$3" "$4"
    runner=$(command -v -- "$5" || printf '%s' "$5")
    [[ ! -f $runner ]] || runner=$(realpath -- "$runner")
    check measurement-runner "$runner" --version
    runner_config=$(jq -cn --arg runner "$runner" '[$runner]')
    for browser in "${browsers[@]}"; do
        driver=$6; key=CHROMEDRIVER; tool=chromedriver
        if [[ $browser == firefox ]]; then driver=$7; key=GECKODRIVER; tool=geckodriver; fi
        if [[ -z $driver ]]; then driver=$(command -v "$tool" || printf '%s' "$tool"); fi
        check "$browser-driver" "$driver" --version
        for context in "${contexts[@]}"; do
            feature=sync
            [[ $context != worker ]] || feature=worker,sync
            check "$browser-$context-snapshot" env -u CHROMEDRIVER -u GECKODRIVER -u CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER "$key=$driver" "CARGO_TARGET_DIR=$PWD/tests/browser/target" cargo test --locked --config tests/browser/.cargo/config.toml --config "target.wasm32-unknown-unknown.runner=$runner_config" --manifest-path tests/browser/Cargo.toml --lib --target wasm32-unknown-unknown --features "$feature" snapshot_materialization_measurement -- --nocapture
        done
    done
    finish

[positional-arguments]
browser-native output="target/browser-validation/native" timeout="600":
    #!/usr/bin/env bash
    source scripts/browser-evidence.sh
    begin "$1" "$2"
    check native-protocol-ownership cargo test --locked --manifest-path tests/browser/Cargo.toml --features sync --test native --test sql_protocol --test kv_protocol
    check native-kv cargo test --locked -p ofdb-kv --features in-memory,redb,sync --test embedded_client --test embedded_redb_client --test local_kv
    check native-sql cargo test --locked -p ofdb-sql --features in-memory,redb,sql,sync,native-stage --test client --test existing --test migrations --test migration_validation --test migration_outcomes --test read_limits --test scan_layout
    check native-runtime cargo test --locked -p ofdb-sync --lib
    check native-clippy cargo clippy --locked -p ofdb-sql -p ofdb-kv -p ofdb-sync-runtime --features in-memory,redb,indexeddb,sql,sync,native-stage -- -D warnings
    finish

[positional-arguments]
browser-final output="target/browser-validation/final" timeout="240":
    #!/usr/bin/env bash
    source scripts/browser-evidence.sh
    begin "$1" "$2"
    check fmt cargo fmt --all -- --check
    check fixture-fmt cargo fmt --manifest-path tests/browser/Cargo.toml --all -- --check
    check clippy cargo clippy --workspace --all-targets -- -D warnings
    check feature-powerset cargo hack test --feature-powerset --all-targets
    check crap just crap
    finish

browser-orchestration-test:
    timeout --kill-after=5 180 bash scripts/test-browser-validation.sh

idp config="config/idp/config.yaml" *args:
    npx portless run --name idp cargo run -p idp-server --features cli --bin idp-server -- --config "{{ config }}" {{ args }}

management config="config/management/config.yaml" *args:
    npx portless run --name management cargo run -p management-server --features cli --bin management-server -- --config "{{ config }}" {{ args }}

device config="config/device/config.yaml" *args:
    npx portless run --name device cargo run -p device-server --features cli --bin device-server -- --config "{{ config }}" {{ args }}

storage config="config/storage/config.yaml" *args:
    npx portless run --name storage cargo run -p storage-server --features cli --bin storage-server -- --config "{{ config }}" {{ args }}

unified *args:
    npx portless run --name unified cargo run -p unified-server -- local {{ args }}

openapi-export:
    cargo run -p unified-server --features independent-contracts --example export-openapi -- openapi

openapi-generate:
    pnpm openapi
