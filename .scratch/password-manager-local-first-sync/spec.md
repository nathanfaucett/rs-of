# Sync the existing Password Manager database

Status: ready-for-agent

## Problem Statement

Password Manager offers **Create vault** and **Create synced vault**. The first writes to the local application database; the second opens a different browser database keyed by a Storage Resource ID. Signing in does not connect existing local vaults to Storage. A User who starts offline must choose between keeping the local vault and starting again in a different store.

## Solution

Keep one active, encrypted Password Manager database per browser profile and one **Create vault** action. A User can create, unlock, and edit vaults without an account. After sign-in, let the User connect the existing database to a Storage Resource. If the signed-in namespace already contains a different Password Manager database and both sides have data, ask before merging. Continue to use the local database when offline, signed out, or unable to sync.

## User Stories

1. As an offline User, I want to create a vault without an account, so I can store and use secrets on my Device.
2. As a signed-in User, I want the same Create vault action, so I do not have to choose a vault type.
3. As a returning User, I want to unlock my local vault without a network or an active sign-in, so a service outage does not block my work.
4. As a User enabling sync, I want to confirm which Installation and User will receive the local database, so I do not upload it to the wrong account.
5. As a User with no remote database, I want to sync the vaults I already created, so I do not have to recreate them or change their passwords.
6. As a User with an existing remote database and an empty browser, I want to load that data into the regular vault list, so I can unlock it as usual.
7. As a User with data on both sides, I want to confirm or decline the first merge, so signing in alone does not combine the databases.
8. As a User who approves a merge, I want both sides' non-conflicting entries retained and conflicts resolved consistently, so all replicas converge.
9. As a User who reconnects to an existing binding, I want sync to resume without another merge prompt, so normal use does not require repeated choices.
10. As a User whose sync fails, I want to see the failure and retry without losing my local edits or generating another Resource.
11. As a User switching accounts, I want the previous sync session to stop and unlocked keys to clear, so the app does not sync my data to another account without consent.
12. As a User with vaults from either old create path, I want to retain access to them after upgrade, so removing the split UI does not strand my data.
13. As a User who syncs secrets, I want Storage to receive ciphertext and not my master password or plaintext secret history.

## Implementation Decisions

- Use one local `ofdb` SQL database for all vaults in the browser profile. Keep vault identity and local unlock independent of sign-in, Resource binding, and sync status. Do not require a Storage request to create or open a local vault. Keep existing vault IDs and master passwords.
- The Device controls the local copy. The remote copy is a Storage Resource in the verified `(User subject, Application)` namespace. An OAuth Client ID or a display name is not a namespace or a Resource identity. Retain the binding to the verified Installation, namespace, and stable Resource ID. Do not reuse a binding across accounts or Installations.
- First attachment is explicit. Identify the destination and scope of upload before sending local changes. Reuse the idempotent Storage registration contract for a missing Resource; do not pick among ambiguous matches by name. If the remote Resource exists and local data does not, reconcile it into the normal local store. If both contain independently created data, require explicit merge approval before either side changes. A declined merge leaves both unchanged.
- Distinguish initial reconciliation of independent databases from normal exchange between established replicas. Use `ofdb` SQL history and its deterministic conflict ordering, not `updated_at` or a new app-level last-write-wins rule. Preserve stable vault and item identities and deletion history. Do not infer deletion from an absent row or match by display name. Prove the merge preserves decryptable complete records, including ciphertext and its nonce; where history or identity is incompatible, stop without discarding either source.
- Preserve existing local and legacy synced data. Reopen or migrate legacy data through a supported, non-destructive path into the one active database. Keep the original data recoverable until the result is verified. Remove the separate synced-vault create and unlock path only when existing users can access their data through the unified flow. Do not clear browser storage or rewrite credentials to force migration.
- Keep encryption before writes to persisted history. Sync ciphertext through the existing SQL history transport. No plaintext secret, master password, or unwrapped key goes to Storage. Metadata protection must not be implied beyond what the schema encrypts.
- Reuse the browser SQL sync lifecycle over the existing Storage WebSocket transport where possible. Attach it to the already-open local database instead of opening another database named after the remote Resource. Use the verified IdP and Storage endpoints rather than deriving one URL from the other. The sync adapter owns attachment, cancellation, status, and retry; vault CRUD does not depend on it.
- IdP authenticates Users and OAuth Clients and supplies tokens. Storage owns the Resource catalog and access, and runs the authorized SQL exchange. Device supplies approval, revocation, and placement policy. Do not bypass the non-seeded Storage policy or treat OIDC sign-in as browser Device approval. Sign-out or revocation ends authorized sync, not local custody.

## Testing Decisions

- Use the Password Manager browser UI and the public Storage contract against a real, non-seeded unified backend as the primary test seam. Extend the existing Password Manager and Portless first-run browser fixtures. Assert visible behavior and retained data, not internal method calls, transport frame counts, or database filenames.
- Main acceptance path: create and edit a vault offline, sign in, confirm attachment, and sync the same vault to one Storage Resource. Verify the Resource contains ciphertext and no known plaintext secret or master password. Restart or reload, remove the network or sign out, and unlock and edit the same vault locally. Reconnect and verify the retained edits reach the same Resource without a duplicate or another merge prompt.
- Exercise the empty-local and populated-local cases against an existing Resource. Verify that declining a first merge changes neither side. For an approved merge, compare vault and item identities, non-conflicting entries, a deterministic conflict winner, a deletion, and successful unlock and decryption after reopen. Use the public `ofdb` interface for focused history cases that the browser test cannot isolate reliably.
- Check a denied or lost sync session and an account switch. Verify no unauthorized upload, no local data loss, and retry against the same authorized Resource. Include existing local and legacy synced fixtures to prove compatibility without keeping two active vault paths.

## Out of Scope

- Cross-User sharing, Application-only Storage namespaces, key distribution, and key rotation.
- Browser KV or filesystem transport, transparent peer forwarding, and continuous real-time delivery.
- Full two-Device offline convergence qualification. That is separate from testing initial merge and single-Node reattachment.
- Changing the local master-password model or persisting plaintext secrets.

## Further Notes

- Advances the single-Node workflow in [M2](../../docs/goal.md#m2--single-node-encrypted-synchronization) and prepares for M3. Current code supports explicit SQL exchange on the separate synced path. It does not qualify first attachment of the existing local database, first merge, or two-Device offline convergence. The non-seeded WebSocket path checks the serving Node's Resource Selection; this is not browser Device approval.
- Future sharing: keep one local vault list, but do not grant a recipient access to the entire personal sync Resource. Under Resource-level authorization, a shared vault needs its own Application-scoped Storage Resource with explicit User grants even if the path has no User segment. Use a random per-vault encryption key, protected by the creator's local unlock credential and separately wrapped to each recipient's verified key. Clear unlocked keys on User switch. Keep access grants and key rotation separate from CRDT record conflicts. Revocation cannot recover data or keys already delivered.
