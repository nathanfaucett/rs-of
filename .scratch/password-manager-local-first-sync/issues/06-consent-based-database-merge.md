# 06: Merge independently populated local and remote databases with consent

**What to build:** Detect when an unlinked local database and an existing remote Resource both contain independently created data. Require explicit approval before changing either side, then reconcile them through `ofdb` history with deterministic conflict ordering and one continuing Storage binding.

**Blocked by:** 05 — Run change-driven synchronization in the SDK

**Status:** ready-for-agent

Work status: in progress

Milestone: M2 — Single-Node encrypted synchronization

## Scope and non-goals

- Distinguish first reconciliation of independent databases from normal exchange between established replicas.
- Serialize initial merge with the SDK's normal change-driven sessions so they cannot compete.
- Use `ofdb`'s deterministic conflict ordering, not timestamps or a new application-level last-write-wins rule.
- Match stable vault and item identities, never display names. Do not infer deletion from an absent row.

## Acceptance checks

- [ ] The merge prompt appears before either side changes and identifies the destination and scope of the merge.
- [ ] Declining or cancelling the prompt leaves local and remote data unchanged and does not create a binding.
- [ ] An approved merge retains non-conflicting vaults, folders, and items from both sides and preserves deletion history.
- [ ] Conflicting edits converge to `ofdb`'s deterministic result without consulting entry timestamps.
- [ ] Complete encrypted records remain decryptable after merge and reopen, including ciphertext and nonce.
- [ ] Incompatible identity or history fails closed without discarding either source.
- [ ] The merged database retains one Resource binding and does not prompt again during ordinary reconnection.
- [ ] Public `ofdb` history tests isolate conflict and deletion cases, and the complete consent flow passes through the browser against real Storage.

## Current evidence and qualification limits

The integration branch detects independent local history before restore. The real non-seeded browser test verified cancellation leaves both sources unchanged and does not create a binding. Approved merge remains disabled.

## Comments

The unmerged staging prototype (`2eeabb28`) tests duplicate application IDs, deleted claims, and protected ciphertext groups. It also confirms that two databases initialized by the same migration runner can record equivalent migrations under different Logical Row IDs. Staging currently rejects that case. Do not enable merge until staging can reconcile equivalent ledgers without losing divergent history and verify that encrypted records reopen correctly.

Publication must compare each source with the exact staged snapshot. The Storage transaction must reject concurrent remote writes and save a receipt with the merge. The guarded browser transaction must preserve concurrent local edits and establish the binding only after a verified receipt. Redb and IndexedDB cannot commit atomically together: if Storage commits but local state changes, report that partial outcome and preserve local edits. A lost reply requires verification of the exact operation, not inference from remote history or fallback to ordinary sync.
