# 05: Add change-driven synchronization to SDK database handles

**What to build:** Make explicitly attached SDK database handles observe `ofdb` commit notifications and automatically reconcile committed changes through their configured Storage transport. Put scheduling, session serialization, cancellation, and status in the shared SDK sync lifecycle so applications do not implement their own push loops.

**Blocked by:** 04 — Preserve and protect the Storage binding across sessions

**Status:** ready-for-agent

Milestone: M2 — Single-Node encrypted synchronization

## Scope and non-goals

- The application authorizes attachment and supplies verified endpoints and token access. The SDK must not upload merely because credentials exist.
- Use the existing authorized Storage SQL transport and public database-handle interface; do not add a Password Manager-specific transport or commit watcher.
- Share the lifecycle across browser and native handles where their public contracts support it.
- Change-triggered reconciliation is not a claim of continuous real-time delivery or durable remote replay.

## Acceptance checks

- [ ] Committed creates, edits, and deletes through the normal SDK interface converge to the attached Resource without a consumer calling sync after every write.
- [ ] Bursts produce one active session, and a commit received during that session schedules a follow-up exchange without polling, overlap, or lost wakeups.
- [ ] Closing or detaching a handle cancels its session and prevents later upload.
- [ ] Failed exchanges retain local history and retry on reconnection, a later commit, or an explicit sync request against the same Resource.
- [ ] Password Manager performs ordinary vault CRUD without an app-level push loop, and real-backend Storage still receives only ciphertext.
- [ ] Public SDK tests cover change-driven behavior with real Storage, and the Password Manager consumer flow passes without reload or manual per-edit synchronization.

## Current evidence and qualification limits

The shared SDK lifecycle currently runs explicit SQL exchanges. Password Manager invokes sync after legacy synced creation and unlock. Commit-driven scheduling is not implemented or qualified.
