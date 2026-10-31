# Ticket 02 implementation and validation

## Branch and scope

- Worktree: `/home/nathan/code/rust/aicacia/of-owner-local-lifecycle`.
- Branch: `unified-owner-lifecycle-02`; confirmed starting HEAD and integration base `d6ea7373f`.
- Resumed the interrupted, already-claimed implementation without replacing it. It already contained the bulk lifecycle deletion, owner worker wiring, native compile-critical changes and two replacement test files.
- Six pre-existing tracked `packages/storage-client/node_modules/.bin/*` launcher changes contain only worktree-specific generated paths. They are preserved locally, excluded from the implementation commit, and not treated as source changes.
- Complexity: low in composition, moderate in shared TLS/worker lifetime wiring. Risk: high at the provisioning/trust/retained-state boundary, mitigated by reusing the actual owner workers and refusing old state rather than migrating it. Priority: prerequisite for tickets 03/04.

## Implementation

- One composed local HTTPS endpoint, one LocalTls authority. Three IndependentSetupHost/IndependentCreateHost instances retain independent capabilities and exact Create/Join operations. Owners receive the shared authority/client instead of creating their own authority.
- Expose the existing owner worker entry points to both their standalone launchers and the composer. Shared TLS clients are used for public owner discovery, challenge-bound registration, peer discovery, Device policy and Storage clients, including restart.
- Device publishes its existing serving identity connection/control plane to the in-process Management router. Management uses its supplied public base/prefix and the owner-configured authenticated control plane/client; no fourth setup worker, extra operator registration step, host credential bootstrap or endpoint credential file is introduced. A source-offline permission decision is checked in the focused live test.
- CLI Create/Join take the four base APIs, three protected owner-code paths and the one CA path. Only the IdP request receives the administrator password; each owner request carries exactly that owner's capability. Management is observed at its supplied base after owner admission, not provisioned through a new host setup endpoint.
- Delete host journals/SQL metadata, LocalProvisioner/LocalJoin, bootstrap config/verification, CredentialBootstrap, endpoint credential abstraction and old runtime/worker. Shared owner credential/signer journals remain owner responsibility.
- Previous retained host layouts are refused with preservation/recovery/new-root guidance, including orphaned old endpoint directories; no migration, alias, compatibility wrapper or automatic re-provisioning.
- The host test surface is exactly `tests/composition.rs` (one test) and `tests/live_reconciliation.rs` (one focused test). [Deleted coverage inventory](ticket02-deleted-coverage.md) records intentional drops as well as owning seams. The offs in-process suite is unchanged.
- Native Rust callers, status types and stopped-owner reset calls are migrated only as needed to remove deleted host APIs and keep the workspace compiling. Full frontend/desktop/API-description/document/sample migration remains ticket 04. The opt-in cross-layout harness still describes/calls the old Unified workflow; its scenario migration and filesystem expansion remain ticket 03. No passing cross-layout qualification is claimed here.

## Approved seams and red/green

Approved by spec: composition; existing owner-local contracts; existing cross-layout production deployment suite. The ticket additionally permits one focused live-reconciliation file.

1. The interrupted implementation's existing composition and live reconciliation tests both passed on resume; no retrospective red claim is made for that work.
2. Extended the composition test with orphaned previous `endpoint` state. `cargo test -p unified-server --test composition -- --nocapture` failed at `orphaned prior endpoint must not authorize fresh owners` before the refusal fix.
3. Added that obsolete root to the startup refusal check. The same command passed, including no newly created owner state and preserved prior state.
4. Extended the focused live test with an ordinary authorized Management permission evaluation after target Join, shutdown of both Nodes and source-offline target reopen. This characterization of the resumed implementation passed without an implementation change.

## Commands and results

Commands run from this worktree via `env -C /home/nathan/code/rust/aicacia/of-owner-local-lifecycle`, unless specified otherwise. Initial focused tests also used `cargo --manifest-path /home/nathan/code/rust/aicacia/of-owner-local-lifecycle/Cargo.toml` from the main root. No shared target override was used.

### Passing

- `cargo test -p unified-server --test composition --test live_reconciliation -- --nocapture`: 2 passed after an initial 240-second compilation timeout; resumed completed compilation and both tests.
- `cargo test -p unified-server --test composition -- --nocapture`: red then green as above.
- `cargo test -p unified-server --all-features -- --test-threads=1`: 2 passed, no ignored tests; zero unit/doc entries is not extra coverage. First attempt timed out at 240 seconds after compilation and composition under competing builds; serial retry passed.
- `cargo test -p unified-server --test live_reconciliation -- --nocapture`: passed after adding source-offline Management authorization (approximately 58 seconds).
- `cargo check --workspace --all-targets --all-features`: passed, including native app compile-critical callers. Existing tsify deprecation warnings remain.
- `cargo test -p api -- --test-threads=1`: 45 passed, 2 intentionally ignored child entries, 1 compile-fail doctest passed. Includes shared-authority owner reset isolation.
- `cargo clippy -p unified-server --all-targets --all-features --no-deps -- -D warnings`: passed, also rerun after the CLI change.
- `cargo clippy -p api --all-targets --no-deps -- -D warnings`: passed.
- `cargo run -p unified-server --features cli --bin unified-server -- create --help`: built and passed; exposes all four bases, three codes and single CA path. This is a CLI interface/build smoke check, not production-process Create qualification.
- `cargo fmt --all -- --check`: passed (formatting also applied with `cargo fmt --all`).
- `git --no-pager diff --check`: passed.
- In sibling `offs`: `cargo hack test --feature-powerset --all-targets` and `cargo hack test --feature-powerset --workspace --all-targets`: both passed; no filesystem source changes.

### Broader gates / limits

- `cargo clippy -p unified-server -p api -p idp-server -p device-server -p storage-server --all-targets --all-features --no-deps -- -D warnings`: failed in existing Storage test code, including unchecked read amounts in device_client.rs/idp_client.rs/resources.rs, needless range loops in replication_runtime_tests.rs and manual_async_fn in storage_socket.rs (15 Storage test errors in the final compiler summary). Those assertions/lines were not changed; no unrelated lint cleanup or suppressions.
- `cargo test -p idp-unified --test owner_boundaries owner_composition_tests -- --test-threads=1`: initial 240-second attempt timed out in compilation; a later 180-second attempt timed out while executing owner Join/history/import. No completed suite pass is claimed.
- `cargo hack test --feature-powerset --all-targets`: failed in the first all-feature root-package combination: `owner_boundaries` aborted with a Tokio worker stack overflow (SIGABRT); an associated generated-client child then reported ECONNRESET. This is not a green powerset run and was not baseline-classified in this turn.
- `cargo hack test --feature-powerset --workspace --all-targets`: timed out at 180 seconds while building combination 7/194 (`idp-db`, default feature); not full workspace qualification.
- `just crap`: SDK prerequisite proceeded, then timed out at 180 seconds in instrumented workspace compilation (about 568/975 build items). No CRAP score was produced.
- `cargo test -p lidp --lib -- --test-threads=1`: timed out at 180 seconds compiling native dependencies; no native runtime test pass is claimed. An earlier combined Unified/native command timed out during the preceding Unified test and never reached the native command.
- The old cross-layout Unified HTTP/aggregate setup harness is intentionally not claimed as passing. Ticket 03 must migrate its launch/admission/recovery path before deployment acceptance. Full filesystem deployment, GUI, real credential provider, OS trust installation, non-Linux and production network qualification were not run.

## Final reruns

- After the last CLI/live-test changes, `cargo test -p unified-server --all-features -- --test-threads=1` passed both tests, including source-offline Management authorization (about 61 seconds for the live scenario).
- Final `cargo fmt --all -- --check` passed. The following diff check found one surplus EOF blank line left by deletion of the obsolete root test target; it was removed and the check rerun.
- A source inventory check confirmed all deleted host lifecycle type names absent from the Unified Host Rust tree, thin lib/main modules, and exactly the two intended host test files. This is a source audit, not additional runtime coverage.

