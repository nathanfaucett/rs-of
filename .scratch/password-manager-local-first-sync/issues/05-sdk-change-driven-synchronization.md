# 05: Run change-driven synchronization in the SDK

**What to build:** After explicit attachment, let the SDK observe durable `ofdb` commits on the existing database and schedule `ofdb` history exchange through the authorized Resource transport. Keep scheduling, session serialization, cancellation, retry, and status in the shared SDK so applications do not implement push loops.

**Blocked by:** 04 — Preserve and protect the Storage binding across sessions

**Status:** ready-for-agent

Work status: resolved

Milestone: M2 — Single-Node encrypted synchronization

## Scope and non-goals

- The application authorizes attachment and supplies verified endpoints and token access. The SDK must not upload merely because credentials exist.
- Observe `ofdb` notifications after durable commits, not SDK CRUD helper events. Notifications wake reconciliation; only `ofdb` sync payloads go over the Storage transport.
- Use the existing SQL database and the Resource-scoped transport from attachment. Do not require a database wrapper, a Password Manager-specific watcher, or a new wire protocol. Reuse the lifecycle with native transports where the public contracts support it. Socket multiplexing and browser KV are not required.
- Change-triggered reconciliation is not a claim of continuous real-time delivery or durable remote replay.

## Acceptance checks

- [ ] Committed creates, edits, and deletes through the existing `ofdb` interface reach the attached Resource without a consumer calling sync after each write.
- [ ] Bursts produce one active session, and a commit received during that session schedules a follow-up exchange without polling, overlap, or lost wakeups.
- [ ] Closing or detaching Resource sync cancels its session and prevents later upload without closing the app's local database.
- [ ] Failed exchanges retain local history and retry on reconnection, a later commit, or an explicit sync request against the same Resource.
- [ ] Password Manager performs ordinary vault CRUD without an app-level push loop, and real-backend Storage still receives only ciphertext.
- [ ] Public SDK tests cover change-driven behavior with real Storage, and the Password Manager consumer flow passes without reload or manual per-edit synchronization.

## Current evidence and qualification limits

Merged in `c46801c6`. Real non-seeded Chromium verified encrypted post-commit sync and the delayed cross-tab authorization fence. Browser SQL tests covered cancellation before local apply and before IndexedDB commit. An already-issued commit can still finish. Password Manager requires explicit activation in each runtime. Wakeups cover commits from that WASM runtime, not other tabs, workers, or remote-only writes. The full browser matrix and native qualification did not pass or were not run.
