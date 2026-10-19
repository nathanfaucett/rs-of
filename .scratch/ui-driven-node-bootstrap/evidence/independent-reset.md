# Independent LOCAL host-management reset

Status: implemented source; Rust compilation and runtime qualification NOT RUN. The parent explicitly withheld Cargo permission for this slice. No real CA store or real protected-provider state was modified.

Branch: implement/bootstrap-reset. Base: 68129cb00.

## Scope and risk

Implements the requested explicit-local-reset scope (spec25/40-45/93/102/105). Complexity: moderate (exact owner inventories and a durable deletion state machine). Risk: high (destructive local state and protected secrets). Priority: required recovery path. No broad Cargo Hack, CRAP, platform sweep, or privileged installation was run.

The independent commands are reset-independent --remove-os-trust on idp-server, management-server, and storage-server. They use the same selected config/environment and independent data-dir resolution as ordinary owner startup. The flag is required; there is no browser reset endpoint, no new operator credential, and no default reset/repair on startup. Frontend/native profile reset remains separate. Local reset does not claim remote revocation or remote data deletion.

## Implementation

- New api/src/independent_reset.rs owns stopped-host management, exact public evidence retention, OS trust removal, and staged local deletion. Each owner has a new independent_reset.rs validator and reset entry point. CLI run.rs and thin lib exports wire them.
- Existing independent_create.rs / independent_join_worker.rs worker record files and api/src/independent_create.rs are not edited by this slice.
- IdP/Management reset validators target the readiness-agent CURRENT version-2 schemas (574075709): Join omits IdP credential and Management Create seed. No version-1 compatibility, conversion, or identity regeneration is added. Storage remains version 1. These validators require that readiness slice to be integrated before lifecycle qualification.
- The existing retained setup-host/operation.lock is acquired nonblocking and held through deletion. A live setup-only listener therefore excludes reset even when no domain/runtime has been created or the worker has failed. Missing original operation locks are not repaired.
- worker.json, initialized.json, exact admitted intent, host owner/domain, and matching PEM/DER CA fingerprint are captured. Missing initialized worker evidence, missing admission, unknown owner schema, changed evidence, or unrelated selected roots refuse without synthesizing replacements.
- Raw provider deletion addresses only exact captured consumer IDs and the Management endpoint slot in namespace independent-setup:<absolute setupdir>. An unavailable provider is an error, not successful absence. An attempted missing slot is idempotent on explicit reset; an unattempted slot containing material is a contradiction. No provider enumeration or prefix deletion exists in production.
- IdP reuses its existing exact local secret-inventory deletion contract for Node signing material and metadata references. It opens only an existing database via from_database, without schema initialization, runtime creation, peer access, key generation, or consumer-secret loading. Captured inventory must agree with worker Client references and the selected issuer/namespace.
- Protected deletion succeeds FIRST. A durable owner-only sibling setup-reset.json records secrets -> trust -> data -> cleanup. Provider failures leave all owner/setup files and references; data-cleanup retry does not require already-deleted domain metadata or recreate keys.
- Trust removal is explicit and precedes removal of local TLS/evidence. This base had installation but no existing trust-uninstall helper, so the new module supplies the inverse local operation. Linux uses exact CA PEM with trust anchor --remove, retries via pkexec for native consent, verifies exact fingerprint-named distro-anchor contents before removing only that anchor, and refreshes supported distro stores even when an earlier attempt removed the anchor but refresh failed. macOS uses exact retained certificate and SHA-256 keychain fingerprint, never a broad common-name deletion. OS command denial/unavailability preserves local CA evidence. Actual native consent/platform behavior is not certified by these unrun injected-command tests.
- After domain deletion the setup directory is atomically retired to setup-reset, then removed. The sibling inventory survives partial setup-tree deletion; a retry may create a cleanup-only lock inside the retired tree, never replace a live host lock. A new live setup cannot be confused with the retired identity because the retained CA fingerprint is part of the evidence. Unrelated owner roots, source data, local unrelated provider slots, and foreign namespaces are preserved.

## Qualification source added (NOT execution evidence)

New unified-server/src/independent_reset_tests.rs extends the existing bounded production independent Create/Join test children. Existing lifecycle test files receive only mock-provider/command setup and reset-qualification hooks; their production worker records are not changed.

- Actual completed Create and approved Join, including offline reopen, feed all three owner-local reset entry points.
- Protected-provider absence refuses while retaining worker/domain/TLS evidence.
- The mock provider fails actual endpoint credential deletion after the Management consumer has already been removed. Retry accepts the exact missing consumer and removes the remaining endpoint, without new identities/material.
- A filesystem permission failure exercises data cleanup after secrets/trust succeeded. Retry uses retained evidence and removes only the selected local root.
- Complete provider snapshots verify that uncaptured/source slots survive, including an unrelated slot in the selected namespace and the same Client slot in a different namespace. Target signer/consumer/endpoint slots must be absent after success.
- An additional isolated Storage host accepts actual HTTPS Create, fences a secret attempt, encounters provider failure before domain creation, and retains its operation lock while still listening. Stopped reset with provider absent preserves evidence; provider restoration permits explicit reset without creating the missing key.
- API tests cover explicit trust consent, live setup-lock exclusion, failed trust removal preserving CA/evidence and retry, foreign owner mismatch, lost initialized worker refusal, and no admission regeneration. Each owner has a setup-only live-lock test; each CLI parser tests required explicit trust removal.
- Child PATH contains only fixture trust and pkexec stubs; native roots are a private test bundle. No test delegates to real trust/elevation commands, and no timeout was increased. Permission-failure qualification assumes the specified non-root Ubuntu operator.

## Validation performed

Only scoped rustfmt (edition 2024, skip_children=true) on changed Rust files and git diff --check. Both returned exit status 0. Formatting parses syntax; it does NOT establish type correctness or test success.

No Cargo command was executed. Thus there is no observed red/green TDD run, passing Rust test claim, Clippy result, or actual Create/Join/reset qualification result. TDD skill was loaded, public CLI/owner reset and actual isolated lifecycle seams were used, and initial API/Storage tracer tests were written before their implementation; execution remains parent-gated.

## Parent validation after Cargo grant and readiness integration

Run serially with the existing resource bounds, from this worktree:

    CARGO_BUILD_JOBS=1 cargo test -p api independent_reset::tests -- --nocapture
    CARGO_BUILD_JOBS=1 cargo test -p idp-server -p management-server -p storage-server local_reset_requires_explicit_os_trust_removal -- --nocapture
    CARGO_BUILD_JOBS=1 cargo test -p idp-server -p management-server -p storage-server reset_cannot_remove_a_running_setup_only -- --nocapture
    CARGO_BUILD_JOBS=1 cargo test -p unified-server production_independent_create_and_offline_reopen -- --nocapture
    CARGO_BUILD_JOBS=1 cargo test -p unified-server production_independent_join_and_source_offline_reopen -- --nocapture

Use parent-approved features/target directory when required by the shared build environment. Run the wrapper tests, not the ignored children directly: the wrappers supply isolated native roots, mocked trust/elevation commands, protected-provider process isolation, and the unchanged 90-second bound.

## Integration

Integration merge provenance is recorded in git history. At source implementation time integration/ui-driven-node-bootstrap still points to 68129cb00; readiness is committed separately as 574075709 and its version-2 schemas are a prerequisite for reset lifecycle qualification. This receipt is not a declaration that the broader bootstrap spec is complete.
