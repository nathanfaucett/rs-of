# 06: Merge independently populated local and remote databases with consent

**What to build:** Detect when an unlinked local database and an existing remote Resource both contain independently created data. Require explicit approval before changing either side, then reconcile them through `ofdb` history with deterministic conflict ordering and one continuing Storage binding.

**Blocked by:** 05 — Run change-driven synchronization in the SDK

**Status:** ready-for-agent

Work status: resolved

Milestone: M2 — Single-Node encrypted synchronization

## Scope and non-goals

- Distinguish first reconciliation of independent databases from normal exchange between established replicas.
- Serialize initial merge with the SDK's normal change-driven sessions so they cannot compete.
- Use `ofdb`'s deterministic conflict ordering, not timestamps or a new application-level last-write-wins rule.
- Match stable vault and item identities, never display names. Do not infer deletion from an absent row.

## Acceptance checks

- [x] The merge prompt appears before either side changes and identifies the destination and scope of the merge.
- [x] Declining or cancelling the prompt leaves local and remote data unchanged and does not create a binding.
- [x] An approved merge retains non-conflicting vaults, folders, and items from both sides and preserves deletion history.
- [x] Conflicting edits converge to `ofdb`'s deterministic result without consulting entry timestamps.
- [x] Complete encrypted records remain decryptable after merge and reopen, including ciphertext and nonce.
- [x] Incompatible identity or history fails closed without discarding either source.
- [x] The merged database retains one Resource binding and does not prompt again during ordinary reconnection.
- [x] Public `ofdb` history tests isolate conflict and deletion cases, and the complete consent flow passes through the browser against real Storage.

## Current evidence and qualification limits

Merged in `ba3ca601`. The fresh non-seeded Chromium flow covered decline, approval, a concurrent local edit, lost receipts, durable cancellation, and reattachment. Focused SQL and Chromium tests covered deterministic selection of complete encrypted records in both merge orders and retention of tombstones. Incompatible or ambiguous histories are rejected. Storage and IndexedDB cannot commit atomically together: guarded local completion preserves newer local edits and reports if Storage has already committed. The focused CRAP report flagged `attach_and_sync`; the full workspace suite and browser matrix were not run.
