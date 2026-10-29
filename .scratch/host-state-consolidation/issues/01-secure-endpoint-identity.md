# 01: Put Unified endpoint keys in the secure store

**What to build:** Unified Create and Join persist endpoint private material through the existing secure-provider mechanism rather than an application-managed raw key file. Ordinary startup retains the exact endpoint identity, and explicit reset deletes the exact provider entry. Move generic provider storage beneath IdP-specific repositories only as needed to share the existing mechanism; do not introduce a new repository framework.

**Blocked by:** None (can start immediately).

**Status:** resolved

- [x] Reuse existing byte-secret storage and provider selection, with generic storage independent of IdP signing-key derivation and domain ownership. Preserve appropriate feature gates and default `no_std` behavior for unaffected cryptographic code.
- [x] Unified Create and Join durably record attempted creation before provider work and retain the exact provider reference and expected public endpoint identity. Verify stored material without assuming provider writes are create-only.
- [x] Ordinary and native/shared startup load existing material and verify the retained public identity. Missing/corrupt material, identity mismatch, or provider failure refuses without generating replacement identity or falling back to a file.
- [x] Preserve current endpoint binding, Device approval, and authorization behavior. Transport identity must not depend on an authenticated IdP operation or a domain database that needs the endpoint to start.
- [x] Update all Unified endpoint existence and public-ID checks, reset, and reference recovery. Reset preserves exact references until provider deletion succeeds and supports explicit continuation after failure without deleting unrelated secrets or shared owner data.
- [x] Delete superseded raw endpoint-key creation/read/publication code and obsolete checks. Unsupported retained layouts are refused intact before creating replacement secrets; no migration, dual-write, or compatibility fallback is added.
- [x] Keep this slice independent of the SQL consolidation: use the current authoritative host metadata until its replacement lands, and use the replacement if already available. Do not create a second metadata mechanism to bridge tickets.
- [x] Extend existing endpoint lifecycle and injected-provider tests for fresh creation, stable reopen, attempted-creation interruption, missing/corrupt secrets, provider errors, identity mismatch, and interrupted exact reset. Run affected independent Device/IdP provider regressions when shared code changes.
- [x] Use isolated temporary roots and injected providers, not real secret stores or OS trust. Update affected security, operations, and native lifecycle documentation and remove obsolete behavior in this ticket.
- [x] Record focused test/check results and limits. No full cargo-hack feature-powerset run or CRAP testing is required.

## Comments

- Claimed by endpoint agent on ; base confirmed at 2f3f57c8f, identical to integration tip. Tests use the spec-approved Unified lifecycle and injected-provider seams.

- Reclaimed and actually implemented on 2026-10-29 in `implement/host-endpoint`. The inherited claim above had no implementation. Initial integration merge fast-forwarded `2f3f57c8f` to `3b75fc9db` while preserving this ticket's uncommitted claim.

## Answer

Implemented provider-owned Unified endpoint identities. Generic byte-secret access and platform initialization now live under the feature-gated `key` crate; IdP signing derivation and existing independent provider names/targets remain with their adapters. `key` still compiles without default features; the check exposed missing pre-existing alloc string imports, which are corrected.

Create/Join commit `endpoint_attempted`, the exact service/entry/target and expected public endpoint ID in their **existing authoritative operation journals**, before directory/provider work. No additional metadata file/store was introduced. SQL consolidation has not landed on the integration branch at this implementation's base; ticket 02 remains responsible for replacing these journals. Creation refuses occupied slots and verifies exact readback. Existing-only reopen checks secret length and derived public identity. Missing/uncertain/corrupt/unavailable material never triggers a write, file fallback or replacement identity. Raw endpoint publication/read/pending-key code and owner-only-state fallback startup were deleted. Legacy layouts/operations without references refuse intact.

Stopped reset captures endpoint references through the existing host reset-reference/manifest-backup path, deletes only the exact entry and verifies absence before local data removal. Failure retains existing journals/references; explicit continuation handles already-absent entries without enumeration. Create and Join reset fault tests preserve unrelated secrets. Native/shared provider-safe reset and reference-recovery tests pass. Security, operations, IdP local-security and native lifecycle documentation describe the ownership and intentional format break.

Complexity/risk/priority: medium implementation complexity, high security/recovery risk, high priority. The key risks were accidental fresh creation, overwriting uncertain provider material and dropping reset references. Durable pre-provider attempts, existing-only retry, public-ID/readback verification and retained exact reset evidence address those risks. The provider has no atomic create-only operation: unique references and exclusive host ownership protect cooperating creators; arbitrary external provider mutation is not claimed to be prevented.

### Focused validation (actual commands/results)

All commands ran from `/home/nathan/code/rust/aicacia/of/.git/agent-worktrees/endpoint`. Tests use isolated temporary roots and injected providers/TLS trust operations; no real OS provider or OS trust installation was used.

| Command | Result |
| --- | --- |
| `cargo test -p unified-server --lib local_provisioning_tests::endpoint_private_material_is_provider_owned_and_stable_on_reopen --no-default-features -- --exact --test-threads=1` | TDD red: failed because `endpoint/endpoint.key` existed; green after implementation: 1 passed. |
| `cargo test -p unified-server --lib local_provisioning_tests --no-default-features -- --test-threads=1 --skip https_bootstrap_tests` | Final run: 32 passed, 0 failed; includes nine endpoint/provider lifecycle/fault tests. |
| `cargo test -p unified-server --lib local_provisioning_tests::https_bootstrap_tests --no-default-features -- --test-threads=1` | 7 passed, 0 failed, 1 ignored child entry point (launched explicitly by wrappers). Includes Create/Join/offline reopen, interruption, hosted cancellation/exact retry, unretired finalization and current revocation. |
| `cargo test -p unified-server --lib runtime::tests::native_membership_restores_offline_and_learned_revocation_removes_admission --no-default-features -- --exact --test-threads=1` | 1 passed. |
| `cargo test -p lidp --lib native_shared_create_reopen_and_provider_safe_reset -- --test-threads=1` | 1 wrapper passed; its isolated child passed. Repeated after final provider-reference shape. |
| `cargo test -p lidp --lib setup_reset -- --test-threads=1` | 6 passed. |
| `cargo test -p idp-service --lib provider_isolation_tests -- --test-threads=1` | 3 passed, including independent Device endpoint-slot create/load/refusal/reset regression. |
| `cargo test -p device-service --features std --test device_transport -- --test-threads=1` | 2 passed. |
| `cargo test -p key --no-default-features --features secure-store --lib -- --test-threads=1` | 2 initialization/failure/concurrency tests passed. |
| `cargo check -p key --no-default-features` | Passed. |
| `cargo check -p key --no-default-features --features secure-store,headless` | Passed; compile only, no real provider qualification. |
| `cargo check -p unified-server --test full_node --no-default-features` | Passed after adapting all endpoint fixture initializers. |
| `cargo test -p unified-server --test full_node node_stores_ignore_modifiers_without_crossing_namespaces --no-default-features -- --test-threads=1` | 1 passed. The complete full-node integration suite was not run. |
| `cargo clippy -p key -p idp-service -p unified-server --lib --no-default-features --no-deps -- -D warnings` | Passed. |
| `cargo fmt -p key -p idp-service -p unified-server -- --check` | Passed. |
| `git --no-pager diff --check` | Passed. |

### Failures, prerequisites and limits

- Initial Cargo manifest loading failed because the nested worktree lacked its expected sibling `ofdb`/`ofnet`/`offs` paths. Local untracked worktree-parent symlinks to the existing sibling repositories fixed resolution; no manifest path rewrites or external source changes were made.
- Initial library test compilation exposed existing missing `client_onboarding`/`identity` shared-fixture module wiring. The test-only imports are repaired so the agreed tests can actually compile. Obsolete raw-file and namespace-routing fixtures were rewritten for the provider-owned lifecycle.
- Initial cold builds exceeded 200-second command bounds. A later combined lifecycle invocation exceeded its 900-second bound after waiting for the native build lock; it is **not** counted as a passing suite. The split final lifecycle/HTTPS commands above both completed successfully.
- Earlier implementation-stage failures (fixture argument/port setup, obsolete raw-file/schema expectations, provider fixture routing, missing integration fixture initializer) were corrected and the affected focused tests/checks rerun successfully.
- `cargo test -p device-service --test device_transport -- --test-threads=1` ran zero tests because this target is `std`-gated; the explicit `--features std` command above ran both tests successfully.
- `cargo test -p unified-server --lib runtime::tests --no-default-features -- --test-threads=1`: 2 passed, 1 failed. `builds_three_prefixed_services_with_one_endpoint` reached its JavaScript qualification subprocess, which failed on missing unbuilt `packages/device-client/esm/index.js` (`MODULE_NOT_FOUND`). No unrelated SDK build/artifact changes were made and this broader acceptance test is not claimed as passing.
- Clippy without `--no-deps` failed on existing generated `of-idp-client` lints (97 errors, including `empty_docs` and `derivable_impls`). The explicit affected-package `--no-deps -D warnings` run above passed.
- Existing unused shared-fixture function warnings remain in library test builds. No full workspace suite, feature powerset, `cargo hack`, `just crap`, real OS provider persistence/reboot, real OS trust installation, GUI or platform certification was run or claimed.
