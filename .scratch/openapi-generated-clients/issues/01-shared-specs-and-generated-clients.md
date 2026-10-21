# 01: Check in complete OpenAPI specs and generate both clients

**What to build:** Developers can export complete, reviewable API contracts from the existing Rust definitions and generate working TypeScript and Rust clients from those same local documents without running or provisioning a Node. Use the parent feature spec as the scope and security contract.

**Blocked by:** None (can start immediately).

Status: resolved

Triage: ready-for-agent

- [x] Inventory actual mounted HTTP methods and paths and project-owned outbound API calls; use this inventory to drive completeness and migration, not as the sole proof of router coverage.
- [x] Export deterministic, committed OpenAPI JSON from definitions used by actual routers, including normal owner APIs, public Setup, independent handoffs, host-composed Node-join routes, and applicable discovery and upgrade handshakes. Document non-HTTP protocol limits honestly.
- [x] Export needs no running server, initialized Installation, credentials, or peer. Keep coherent API/topology documents separate where necessary, with correct base and prefix semantics.
- [x] Reuse existing generation tooling. Try its Rust generator first; select another only if a demonstrated contract or transport requirement cannot be met. Pin generation tools.
- [x] Both language clients consume the same committed documents, build successfully, and regenerate reproducibly. Do not hand-edit generated output or independently maintain JSON definitions.
- [x] Prove representative generated operations against actual routes through controlled test listeners, including OAuth form encoding, authentication, required/optional fields, and response/error handling. Verify built JavaScript exports and Rust consumer compilation.
- [x] Prove the Rust integration can preserve required TLS, redirects, proxy policy where applicable, connection/request bounds, and streaming response-size limits before unbounded buffering. Do not defer an unsupported transport requirement to migration.
- [x] Failed schema validation or generation leaves prior SDK artifacts intact. Remove deliberate schema exclusions and superseded live-server generation requirements.
- [x] Keep runtime secrets out of contracts and generated artifacts. Avoid server dependencies in client crates, dependency cycles, and a new general-purpose client framework.

## Answer

Implemented in isolated worktree `of-openapi-contracts`, branch `implement/openapi-contracts`, based exactly on integration commit `35dc1cbe7ae2a51205dbd8b91ec15443fe40d1c5`. Parent worktree was not edited. The approved seams are actual owner/Setup/host-composed router listeners, offline export/generation commands, and the generated Rust transport boundary.

### Implementation evidence

- `openapi/`: ten deterministic canonical documents, 128 documented method/path entries across coherent normal-owner, public Setup, owner-admission, independent handoff/owner-join and unified Node-join APIs. `crates/unified-server/examples/export-openapi.rs` exports production definitions without listeners, Installation state, credentials or peers.
- Independent Setup and new independent/unified join documents use the registrations mounted by their actual routers. Worker challenge/register/history exclusions and obsolete live-server/export-test hooks were removed. Query parameters, JSON-encoded OAuth form authorization details, token headers/errors, Setup schemes, Node-join response differences and upgrade headers/boundaries are described in Rust definitions.
- Existing OpenAPI Generator 7.25.0 / CLI wrapper 2.41.0 is pinned and retained for both languages. `scripts/generate-openapi-client.mjs` consumes the same local documents, stages both languages, validates/formats/compiles, and only then replaces artifacts, with rollback for replacement failure. No generated source was hand-edited.
- Rust SDKs are in independent `clients/rust` workspace with locked dependencies and no server dependencies. Thin crate/module entries are preserved. TypeScript SDKs are integrated into existing owner packages, including built exported subpaths for Setup, independent handoffs/owner joins, owner admission, unified Node-join, and generated Storage operations.
- Checked-in Rust templates retain reqwest 0.12 injection and implement bounded incremental success/error reads, total URL/header/body request accounting, native-root verified HTTPS, no redirects/proxies, and configurable per-caller bounds. Defaults: 5-second connect, 15-second request, 1 MiB byte limits. Application token caching and domain/security validation remain ticket 02 responsibilities, not duplicated generated HTTP operations.
- `docs/openapi-inventory.md` inventories mounted methods/paths and production outbound Rust/browser/native calls, including native lifecycle/revocation and opaque upgrade protocol limits. `docs/api.md`, package commands and just recipes document the local workflow. `scripts/qualify-openapi-clients.mjs` replaces live-server generation qualification.
- Complexity/risk assessment: high-priority contract completeness and transport-security preservation; use existing router machinery, generator, reqwest, node:test and listener fixtures rather than a new client framework. Consumer migration is ticket 02; CI enforcement is ticket 03.

### Validation evidence

Commands were run in the isolated worktree, using external Cargo target directories so parent build artifacts were not touched.

- `pnpm openapi:check`: PASS end to end. Repeated export byte identity, committed-contract equality, both-language regeneration without drift, all ten Rust SDK builds, owner TypeScript builds, built JavaScript package subpath resolution, **31 JavaScript tests**, **five generated-operation/transport Rust tests**, and actual unified Node-join listener qualification passed.
- `cargo test -p api -p idp-server -p management-server -p storage-server --no-default-features openapi`: PASS, **11 tests**, no ignored export hooks remaining.
- `cargo test -p unified-server builds_three_prefixed_services_with_one_endpoint`: PASS after obsolete live-schema hooks were deleted; generated Rust and built JavaScript source Node-join calls exercise the actual composed router.
- Transport suite: PASS with deliberately unusable `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, and empty `NO_PROXY`. Tests reject unknown TLS certificates, plaintext defaults, redirects, malformed JSON, oversized declared and unfinished chunked bodies, oversized request headers, and stalled responses. Chunk overflow returns before stream completion or timeout.
- Generation failure tests: PASS for invalid schemas, generator failure, TypeScript formatting/build failure, and Rust formatting/build failure; both prior SDK families remain byte-identical.
- `cargo hack test --feature-powerset --all-targets`: PASS, **10 root-package feature combinations**. This is the requested root command, not a claim of workspace-wide feature-powerset execution; configurations without the CLI target are reported as no-ops by Cargo.
- `cargo fmt --all -- --check`, generated Rust workspace formatting, and `git diff --check`: PASS.
- `cargo clippy -p api -p unified-server --lib -- -D warnings`: PASS.
- Contract/generated-Rust scan: no absolute worktree paths, private-key material, or runtime-secret test fixtures found in committed artifacts.

### Caveats

- All-target clippy is blocked by 16 existing unified-server test lints (including `api_create_tests.rs`, `config.rs`, `endpoint.rs`, `local_provisioning_tests.rs`, `runtime.rs`, and independent operation tests). Unrelated test code was not rewritten to silence them.
- `just crap` was attempted twice with 300-second bounds and timed out during full-workspace instrumented compilation, including GTK/Tauri/native crates. No completed CRAP score or full-workspace coverage result is claimed.
- Initial tool/dependency acquisition may require network access; normal schema/client generation is local once pinned tools/dependencies are cached. WebSocket/Iroh/SQL/filesystem message protocols and production/native GUI acceptance are not implemented or implied by generated HTTP support.
