# 02-device-server-with-its-own-database.md

Status: resolved

Claimed by: implement/shared-control-plane-02 (base integration/shared-control-plane-databases at 9e7360378).

## Problem Statement

The `management-server` currently owns Device state, including identity, approval, and replication policy. This data is part of a separate write domain from the Identity database. Furthermore, `storage-server` currently depends on the Management database directly (via in-process handles) for replication admission, which prevents scaling the device lifecycle independently and makes the architecture harder to decouple.

## Solution

Extract all device-related logic and data into a new `device-server`. This server will manage its own database containing device identities, endpoint bindings, approvals, revocations, restrictions, and replication policies. The `storage-server` will then query the `device-server` (instead of opening the Management database) to perform replication admission.

## User Stories

1. As a Unified Host operator, I want a second database handle for Devices, passed to the Device server and to Storage admission, so that Device state is not stored in the identity database.
2. As a Device owner, I want Device create, name, approval, transfer, and revocation on the Device server, so that a Device stays a Management-shaped domain identity with one authority.
3. As a Trusted Device, I want Storage to admit replication only while the Device server reports that Device as approved and in policy, so that endpoint possession alone still does not grant synchronization.
4. As a Storage operator, I want replication admission to ask the Device server, so that Storage does not open the Management or Identity databases.

## Implementation Decisions

- The Device database holds: Devices, endpoint bindings, approval/revocation status, restrictions, and selection/replication policies.
- The `device-server` is a standalone service that can be run in-process (via injected `Arc<Database>`) or out-of-process (via `ofdb-sql` gRPC).
- `management-server` is stripped of device-related routes and logic; it only retains role and permission HTTP.
- `storage-server` is updated to call the `device-server` (via HTTP or gRPC) for admission checks instead of reading a local database.

## Testing Decisions

- **Unified Host Test:** Verify that a device can be approved on the Device server and that Storage recognizes it as a trusted device, using the second injected handle.
- **Separation Test:** Verify that device-specific records (like a new device) are not visible when querying the Identity database.

## Out of Scope

- Moving OAuth, token signing, or User credentials onto the Device server.
- Changing how user-data replication (the iroh sync path) works.

## Answer

Implemented on `implement/shared-control-plane-02` in worktree `02`, based exactly on `integration/shared-control-plane-databases` at `9e7360378b36cdd0385d6559afa53ae74034c005`.

### Delivery

- Extracted Device contracts, enrollment/identity/pairing, repositories, migrations, selection policy, and public Device history into `device-service`; extracted Device HTTP, hosted Node joining, and the local Device lifecycle into `device-server`. There is no Management forwarding facade or old-route alias.
- Management HTTP now exposes roles, permissions, assignments, permission evaluation, and metadata only. It has no Device database opener, Device migrations, or Device setup/join HTTP. The old mixed-domain Management startup was removed; standalone remote identity startup is ticket03's responsibility.
- Unified opens the identity database and `device/devices.redb` separately and composes `/idp`, `/management`, `/device`, and `/storage`. Provisioning retains the two owner handles through serving. Device migrations/history exclude identity policy tables.
- Storage receives `DeviceAuthority` through its admission callback, or uses the authenticated generated Device HTTP client. Its admission path opens neither control-plane database. Authority rechecks current endpoint/owner binding after policy reads and around owner-copy cleanup; unknown/revoked/conflicted/inactive owners, missing policy, restrictions, and pending cleanup remain fail closed.
- Preserved exact actor/application permissions, the `idp-management` actor audience, and existing service scope identifiers. Last-approved-Device revocation protection remains intact. Signed independent setup locators and setup owner evidence now name `device`; there is no compatibility decoder for `management` Device locators or old Device reset inventories.
- Added a fourth canonical contract and Rust/TypeScript Device SDKs; regenerated affected SDKs, updated native peer revocation, Device UI/configuration, setup clients, tests, domain glossaries, and stable control-plane documentation.
- No SQL gRPC Device startup was implemented. Local standalone Device startup requires an explicit existing identity database for permission evaluation; out-of-process Device database wiring/qualification remains ticket04.

Complexity is primarily a mechanical ownership move; security risk is concentrated in admission and transfer cleanup. Those checks were retained rather than simplified. TDD started with a Management contract regression that failed on `/device`, then passed after extraction. The new Unified acceptance exercises HTTP enrollment/approval and restrictions, Storage authority, schema isolation, unknown endpoint denial, and learned revocation.

### Validation

Rust commands used `CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target`. Final focused commands also used `CARGO_INCREMENTAL=0` and `RUST_MIN_STACK=67108864`; incremental reuse was disabled after a cached hidden-symbol linker failure, without cleaning the shared target.

Passed:

- `cargo test -p device-service --features replica --lib -- --test-threads=2` — 56 tests.
- `cargo test -p management-server -p device-server --no-default-features -- --test-threads=2` — 31 unit tests and 2 hosted-join integration tests.
- `cargo test -p storage-server --no-default-features --features network --lib local_replication_authorizer -- --test-threads=2` — 4 tests.
- `cargo test -p storage-server --no-default-features --features network --lib device_client -- --test-threads=2` — 11 tests.
- `cargo test -p unified-server --no-default-features --lib device_authority_tests::device_http_and_storage_share_isolated_authority_and_recheck_revocation -- --exact` — 1 test.
- `cargo test -p unified-server --no-default-features --lib runtime::tests::native_membership_restores_offline_and_learned_revocation_removes_admission -- --exact` — 1 test.
- `cargo test -p unified-server --no-default-features --lib permission_http_tests::shared_identity_management_writes_change_idp_authorization -- --exact` — 1 test.
- `cargo test -p unified-server --no-default-features --test composition` — 2 tests.
- `cargo check -p lidp --lib --message-format short` — native Tauri library compilation.
- `cargo clippy -p device-server -p management-server --no-default-features --lib --message-format short` — completed; generated-client/dependency warnings remain, no Device/Management server warnings.
- Production OpenAPI export and `node scripts/generate-openapi-client.mjs` for `idp`, `management`, `device`, and `storage`; generated Rust checks and TypeScript validation completed. All four TypeScript SDKs were built for runtime tests.
- `node --test scripts/owner-contracts.test.mjs scripts/generated-device-client.test.mjs scripts/generated-independent-client.test.mjs scripts/generated-setup-client.test.mjs` — 18 tests.
- `node --test scripts/generated-clients.test.mjs scripts/generated-package-exports.test.mjs` — 17 tests.
- `node --test scripts/generate-openapi-client.test.mjs` — 8 atomic-generation tests.
- `pnpm --dir apps/app exec vitest run --config vitest.auth.config.ts src/lib/common/state/ownerClients.test.ts src/lib/common/state/setupClient.test.ts src/lib/common/state/independentSetup.test.ts` — 106 tests.
- Svelte autofixer checks on changed components/modules; Rust formatting and `git diff --check`.

Limits and unsuccessful broader checks:

- `cargo hack test -p device-service --feature-powerset --all-targets` stopped in combination 2/10: `approved_enrollment_denies_unresolved_owner_conflicts_even_with_exact_visible_binding` failed importing sync state with `Incomplete catalog: data table devices has no definition`. The no-default combination passed, and all 56 feature-enabled unit tests passed before that integration failure. No full feature-matrix or SQL sync qualification is claimed.
- `just crap` timed out after 180 seconds during workspace coverage compilation; no CRAP score was produced.
- `pnpm --dir apps/app run check` reports 28 errors from unavailable workspace package build outputs (`svelte-headless`, `oidc-client`, `native-fetch`) and generated Paraglide modules in this worktree. Full Svelte app qualification is not claimed; the focused suites and component checks passed.
- The broader historical independent-host/default-stack/native-provider suites were not qualified. Remote identity is ticket03; SQL gRPC Device startup and remote Device acceptance are ticket04.

Integration: implementation commit `37a2d51e5` was followed by `GIT_EDITOR=true git merge integration/shared-control-plane-databases` in worktree `02`. Git reported `Already up to date`; the checked integration tip was `9e7360378b36cdd0385d6559afa53ae74034c005`, already an ancestor. No merge was performed into the main working tree.
