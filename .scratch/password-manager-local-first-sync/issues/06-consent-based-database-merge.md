# 06: Merge independently populated local and remote databases with consent

**What to build:** Detect when an unlinked local database and an existing remote Resource both contain independently created data. Require explicit approval before changing either side, then reconcile them through `ofdb` history with deterministic conflict ordering and one continuing Storage binding.

**Blocked by:** 05 — Add change-driven synchronization to SDK database handles

**Status:** ready-for-agent

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

Current code has no consensual first merge of independently populated local and remote Password Manager databases. Deterministic CRDT convergence alone does not qualify this workflow or its compatibility failure behavior.
