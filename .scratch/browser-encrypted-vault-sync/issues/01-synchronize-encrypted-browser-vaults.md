# 01: Synchronize encrypted browser vaults through Storage

**What to build:** A signed-in password-manager User can synchronize an existing encrypted IndexedDB vault through the public sync SDK to its real Storage Resource. Initial secrets, updates, and deletions arrive as ciphertext through SQL history v7; plaintext and the vault master password never reach Storage. The work includes the browser v7 binding, the independent IndexedDB committed-change repair for legacy direct-peer synchronization, readable synchronization errors, focused tests, and current documentation. It does not reset existing local data or change the application's public synchronization workflow.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

Work: open
Milestone: M2 — Single-Node encrypted synchronization
Dependencies: Existing SQL history v7 sessions, IndexedDB SQL persistence, Storage WebSocket synchronization, and real-backend browser runners. No new external service or synchronization package.

## Acceptance criteria

- [ ] Existing IndexedDB vault data opens without deletion, reinitialization, or a storage migration; local-only vault behavior and OIDC sign-in remain unchanged.
- [ ] A public browser SQL operation uses the existing v7 history session with caller-supplied byte transport and bounded in-memory staging. It does not send legacy v1 messages to Storage or require native filesystem/Tokio support.
- [ ] Two independent IndexedDB Databases converge create, update, and delete history in both directions through the public v7 operation. Repeated synchronization does not duplicate Logical Rows.
- [ ] IndexedDB retains the engine's existing committed-change signal, and the existing legacy direct-peer browser synchronization path passes a focused public-API regression test.
- [ ] Sync SDK Storage call sites select the v7 operation while the public Database handle `sync` workflow remains unchanged. Success is reported only after a complete exchange; failures preserve the SDK error state.
- [ ] The real-backend vault scenario creates and synchronizes an encrypted secret, then proves through the public Storage API that one remote ciphertext row exists and neither plaintext nor the master password is exposed.
- [ ] The same scenario rejects a wrong master password, reopens with the correct password, and decrypts the synchronized local record successfully.
- [ ] After a vault update and synchronization, remote ciphertext changes and the updated plaintext remains absent from Storage.
- [ ] After a vault deletion and synchronization, the remote query returns no secret rows. Repeated synchronization remains idempotent.
- [ ] Representative WASM-like errors retain their exposed message and original cause through the SQL error wrapper. No Token, plaintext, ciphertext payload, or arbitrary frame logging is added.
- [ ] SQL WASM/browser tests, sync SDK checks, affected password-manager checks, focused Rust checks, formatting, and appropriate feature checks pass with recorded counts.
- [ ] The root real-backend Vitest run passes both application suites against the rebuilt unified server. The real-backend Playwright suite passes without mocks, skipped synchronization, or local-only substitutes.
- [ ] Current documentation describes only the qualified single-Node behavior. GitHub Actions remains the source of pushed qualification evidence.

## Non-goals

- Two-Device offline convergence, continuous real-time delivery, durable replay, or multihop synchronization.
- Device, Selection, Management, or Storage authorization redesign.
- Push-only/pull-only enforcement; the existing Storage session remains Bidirectional.
- KV or filesystem synchronization.
- A v1 adapter on the v7 route, a new wire version, or a vault/data migration.
- Native Node, independent deployment, desktop, or Tauri qualification.
- Production password-manager qualification or unrelated application features.

## Current evidence and limits

The current real-backend password-manager suite has four passing tests and one failing encrypted-vault test. Runtime diagnostics exposed `apply error: committed changes require a persistent database` before SQL owner messages leave the browser. Static tracing also established that the current browser path sends legacy v1 messages while Storage expects SQL history v7. Both defects are in scope; no existing implementation or validation pass is claimed by this ticket.
