# System tests

Run from the repository root on Linux. These tests launch real binaries and use isolated temporary state. A listed test is not a passing qualification result.

## Available checks

Run the non-ignored private-root regression:

```sh
cargo test -p idp-unified --test system_create create::root_is_private -- --exact --nocapture
```

Run the local unified-server composition tests:

```sh
cargo test -p unified-server --test composition
```

The composition source starts the actual binary with fresh state. It covers local health/discovery, sign-in and PKCE flows, seeded Users, Resource use, restart, and HTTP/TLS variants. A passing run qualifies only that local composition path, not independent deployment or the target shared-Device model.

## Ignored independent-process scenarios

`tests/system_create.rs` contains five serial, opt-in scenarios:

- independent Create and Resource use;
- retained restart;
- permission and Resource denial; and
- current local-owner Create/restart behavior.

They require the `identity-database` binary and an example launcher named `system-test-service` in the same Cargo target profile. Current Cargo metadata contains `identity-database` and `system_create`, but no `system-test-service` example. The scenarios are therefore blocked as checked in; do not report them as qualified.

After that launcher is restored, the intended commands are:

```sh
cargo build -j 8 -p idp-unified --example system-test-service -p idp-server --bin identity-database
cargo test -j 8 -p idp-unified --test system_create -- --ignored --test-threads=1 --nocapture
```

Use the same Cargo target directory and profile for both commands. The scenarios are ignored because they are expensive and launch processes. Run them serially. The non-ignored private-root regression is separate.

## What the process scenarios cover

The source drives generated clients against independent IdP, Device, Storage, and Management entry points. It checks current Create retries, restart, retained endpoint evidence, token/grant denial, and namespace authorization.

Filesystem cases exercise exact file bytes, deletion/listing behavior, local Residency transitions, Resource deletion, and retained restart. Authorization cases include actor/Application substitution, wrong audience, ungranted Device actions, read-only writes, wrong Resource kinds, and denial on an existing connection.

These tests exercise current legacy lifecycle and single-User owner behavior where present. They do not qualify removal of that model, Installation-level Devices shared by multiple Users, Resource ownership after revocation, or completed `rs-of` migration. See [target architecture](../../docs/architecture.md).

## Safety and qualification boundaries

Fixtures use private temporary roots, mock credential providers, and mock OS-trust commands. They do not use a real secure store or install OS trust. Child processes are killed and reaped before normal cleanup. Failure diagnostics can retain private local files; do not publish them.

The scenarios use bounded startup/request/protocol waits and reduced test password cost. They do not qualify production password settings, WAN behavior, default stack sizes, real OS trust, remote filesystem content fetching, or every crash/recovery point.

One-process local composition is not evidence of multi-Node replication. Device revocation tests are not evidence of Resource ownership transfer or remote erasure; neither is a target guarantee. Database-file layout in one test is not a requirement for every deployment.

[e2e/README.md](../../e2e/README.md) documents browser/backend commands. GitHub Actions records current and historical results.
