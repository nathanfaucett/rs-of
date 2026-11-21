# Synchronize encrypted browser vaults through Storage SQL history v7

Status: ready-for-agent
Work: resolved
Milestone: M2 — Single-Node encrypted synchronization
Dependencies: Existing SQL history v7 sessions, IndexedDB SQL persistence, Storage WebSocket synchronization, and real-backend browser runners. No new external service is required.

## Problem Statement

A password-manager User can create and use an encrypted vault locally and can sign in to the local Sync Server, but cannot synchronize that vault to its Storage Resource. The real-backend vault test reaches synchronization and fails before remote ciphertext can be checked.

The immediate failure occurs in the browser before SQL history leaves the Device. IndexedDB Database construction discards the engine's committed-change signal, while the current synchronization call requires that signal. Its raw WASM error is `apply error: committed changes require a persistent database`; the TypeScript wrapper hides this message as `[object Object]`.

The current call path also selects the wrong SQL protocol. The browser uses legacy push synchronization v1, while the Storage WebSocket route advertises and accepts SQL history v7. The local signal failure prevents the current test from reaching this second incompatibility.

## Solution

A User who signs in to the Sync Server can synchronize an existing encrypted vault without recreating it or exposing its master password or plaintext. Initial secrets, updates, and deletions reach the User/Application Resource through the existing Storage API and SQL history v7 session. Repeating synchronization is safe, and failures remain visible instead of reporting a successful sync.

The browser SQL Database retains its existing committed-change signal for the legacy peer-synchronization contract. Storage synchronization uses a distinct browser binding for the canonical v7 history session rather than sending legacy push messages over a v7 route. The public password-manager and sync SDK workflow remains unchanged.

## User Stories

1. As a password-manager User, I want my local encrypted vault to remain usable without an account, so that enabling synchronization does not change local-only behavior.
2. As a password-manager User, I want to synchronize after OIDC sign-in, so that my encrypted vault has a Replica on my Sync Server.
3. As a password-manager User, I want newly created secrets to reach the Storage Resource, so that the remote Replica contains the same vault records.
4. As a password-manager User, I want updated secrets to replace the remote ciphertext, so that the Sync Server does not retain only the initial version.
5. As a password-manager User, I want deleted secrets to remain deleted after synchronization, so that an older Replica cannot restore them.
6. As a password-manager User, I want only ciphertext on the Sync Server, so that Storage never receives my plaintext secrets or vault master password.
7. As a password-manager User, I want the correct master password to reopen my vault after synchronization, so that synchronization does not damage local encrypted data.
8. As a password-manager User, I want a wrong master password to remain rejected, so that enabling synchronization does not weaken vault encryption checks.
9. As a password-manager User, I want repeated synchronization to be idempotent, so that retries do not duplicate vault records.
10. As a password-manager User, I want synchronization failures to be visible and understandable, so that I am not told that a failed operation completed.
11. As an application developer, I want the existing sync SDK Database handle workflow to remain stable, so that application call sites do not need a new synchronization state machine.
12. As an application developer, I want existing IndexedDB data opened without deletion or reinitialization, so that enabling the repair does not destroy a local vault.
13. As a SQL package maintainer, I want Storage synchronization to use the same v7 history implementation as the Rust Storage client and server, so that clients do not maintain incompatible wire state machines.
14. As a SQL package maintainer, I want legacy direct-peer push synchronization to retain its explicit contract, so that fixing Storage integration does not silently change an existing public operation.
15. As a browser maintainer, I want v7 staging and transport limits enforced without native filesystem or Tokio dependencies, so that synchronization remains bounded and browser-compatible.
16. As a maintainer, I want the real application test to inspect remote ciphertext after create, update, and delete, so that a mounted route or local-only write cannot qualify the feature.
17. As a maintainer, I want useful typed errors without token, plaintext, or frame-payload logging, so that failures are diagnosable without exposing secrets.

## Implementation Decisions

- Own browser persistence and synchronization lifecycle in the SQL Database facade. Do not implement vault-specific synchronization in the password manager.
- Retain the SQL engine's existing committed-change signal when constructing an IndexedDB Database. Reuse the existing commit notification path; do not add a second event stream or derive outbound changes from application queries.
- Add a distinct SQL history v7 browser operation through the Database and WASM bindings. It must use the existing history session, caller-supplied byte transport, initiator role, and bounded in-memory staging. Do not use native file staging, native sockets, Tokio, or a second protocol implementation.
- Keep the legacy push synchronization operation and its configuration/result contract separate from the v7 history operation. Do not silently reinterpret legacy push configuration as history configuration.
- Update sync SDK call sites that target Storage to use the v7 history operation. Preserve the public Database handle `sync` workflow and its existing success/error state behavior. Updating a call site does not qualify another runtime or deployment mode.
- Preserve the existing Storage Hello/Attach framing, `storage:sync` Token exchange, Resource Namespace checks, and SQL responder. The browser must not send legacy v1 owner messages on a route that declares SQL history v7.
- Keep the current Bidirectional Storage attach behavior. The requested convenience direction is not evidence of push-only enforcement; directional policy is not changed by this work.
- Use the existing v7 session defaults and outer WebSocket frame/session limits. Do not add unlimited buffering or relax message validation to make the vault test pass.
- Report success only after the complete v7 exchange succeeds. Transport, authorization, protocol, staging, and apply failures must reject the operation and leave the SDK in its error state.
- Convert exposed WASM error messages into the existing SQL package error without discarding the original cause. Do not log access Tokens, vault secrets, ciphertext payloads, or arbitrary WebSocket frames.
- Preserve existing IndexedDB schemas and retained history. No data reset, vault migration, new storage format, or v1-to-v7 wire adapter is required. Tests must not clear a developer's existing vault or browser profile.
- Keep the Storage server contract unchanged unless runtime evidence identifies a separate defect after the browser uses v7. Any such defect must be fixed at its owning context with an explicit compatibility decision.
- Update current browser and synchronization documentation after the real-backend test passes. Do not claim two-Device convergence, continuous delivery, durable replay, or production password-manager support.
- Reuse existing dependencies and generated WASM workflows. Do not add a JavaScript reimplementation of SQL history or a new synchronization package.

## Testing Decisions

- Use the existing real-backend password-manager Vitest browser suite as the primary and highest acceptance seam. It exercises the public application SDK, real WASM, real OIDC/Storage HTTP APIs, and the real WebSocket route. Do not replace it with mocks, direct HTTP writes, skipped synchronization, or local-only assertions.
- Preserve and pass the existing encrypted-vault scenario: create a local encrypted vault, synchronize, inspect one remote ciphertext row, reject a wrong password, reopen with the correct password, synchronize an update, inspect changed ciphertext, synchronize a deletion, and inspect an empty remote result.
- Strengthen that external scenario only where needed to prove repeat synchronization is idempotent and no remote response contains the plaintext or master password. Do not assert private protocol calls or internal engine state.
- Use the existing public SQL browser binding tests as the focused regression seam. Exercise independent IndexedDB Databases through caller-supplied transports rather than adding test-only engine hooks.
- Add a focused v7 public-browser test that exchanges create, update, and delete history in both directions between two updated Databases and verifies repeat synchronization does not duplicate Logical Rows. Reopen persisted data where the existing test harness supports it.
- Add a focused legacy peer-synchronization regression because that public operation remains supported. It must prove IndexedDB commits are available to the existing legacy operation; this covers the retained committed-change signal without testing a private field.
- Add public SQL error-wrapper tests using representative WASM-like error objects. Verify the supplied message and cause survive wrapping and no payload logging is introduced.
- Reuse existing Rust SQL history session tests for protocol invariants. Add a focused test only for new facade behavior that the browser tests cannot isolate; do not duplicate the full v7 state-machine suite.
- Run the SQL WASM build and browser package tests, sync SDK build/tests/type checks, and affected password-manager checks. Generated artifacts must come from the changed Rust sources before browser validation.
- Run the root real-backend Vitest command against the rebuilt unified server and require both application suites to pass. Then run the real-backend Playwright suite to check OIDC and Storage HTTP regressions.
- Run focused SQL and Storage Rust tests, formatting, Clippy and the appropriate feature-powerset checks for the changed Rust surface. Run documentation and whitespace checks after contract updates.
- Record exact test counts and any unrun broader qualification. Do not infer hosted CI success from local results; GitHub Actions records the pushed result.

## Out of Scope

- Two-Device offline convergence, automatic real-time delivery, durable replay, or transparent multihop synchronization.
- Device ownership migration, Selection redesign, Management policy changes, or new authorization grants.
- Push-only or pull-only Storage enforcement. The existing server grants Bidirectional sessions.
- KV or filesystem synchronization and their browser APIs.
- A legacy v1 adapter on the Storage v7 route, a new wire version, or changes to established v7 message schemas.
- Migration or deletion of existing IndexedDB/vault data.
- Native Node, independent deployment, desktop, or Tauri qualification.
- A production-qualified password manager or unrelated password-manager features.
- Broad cleanup of legacy service-crate lint debt or unrelated synchronization APIs.

## Further Notes

Runtime reproduction on the current main branch produced four passing password-manager backend tests and one failing encrypted-vault test. Instrumentation exposed the local committed-change error before any SQL owner exchange. No server authorization failure was observed at that point.

Static protocol tracing independently established that the current browser call sends legacy v1 messages while Storage expects v7 history. Both defects must be addressed; fixing only the IndexedDB signal is insufficient.

The investigation used temporary diagnostics only. Those edits were removed, and no implementation or validation pass is claimed by this specification.
