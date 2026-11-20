# Product goal

## Vision

A User must be able to keep application data on their Devices, work without a network, and synchronize through infrastructure they control. Identity and authorization must not require a permanent connection to a central authority.

The first product path is deliberately small: one encrypted password vault, one User, one Application, and two Devices. SQL synchronization and Device administration must work for this path before the platform expands into more resource types and SDKs.

This file contains target outcomes. [README](../README.md#current-status) records current support.

## North-star workflow

1. A User creates an encrypted vault on Device A.
2. The password-manager reference application reads and writes that vault without an account or network.
3. The User signs in to their Sync Server with OIDC.
4. The Sync Server authenticates the User and Application. Device A receives separate approval to synchronize.
5. The User or an authorized administrator selects the vault Resource for Device A and the Sync Server.
6. The application synchronizes ciphertext. The Sync Server does not receive the vault master password.
7. The User signs in on Device B, approves that Device, and selects the same vault Resource.
8. Both Devices edit offline. Their changes converge after they reconnect.
9. Revoking Device B prevents later synchronization after the revocation is learned. Revocation does not transfer Resource ownership or promise remote erasure.

A Sync Server is a Node that exposes HTTPS identity, policy, Device, and Storage APIs. It is an operational role, not a separate authority in the domain model.

## Domain rules

- Resources belong to a `(User subject, Application)` namespace.
- A Device is an approved Installation endpoint, not a User-owned data principal.
- One Device may hold authorized replicas for several Users and Applications.
- Selection states which Resource should have a replica on which Device.
- Management permissions authorize administration. Device service owns Device records. Storage owns Resource catalogs and access.
- OIDC authenticates API access. It does not replace Device approval or Selection.
- Local vault encryption is separate from OIDC. A bearer token is not a vault decryption key.

See [architecture](architecture.md).

## Milestones

### M1 — Canonical foundation

- One canonical checkout and reproducible CI.
- One product goal, architecture, glossary, and current-status view.
- Documentation links and examples checked automatically.
- No manual synchronization with sibling source repositories.

### M2 — Single-Node encrypted synchronization

- The reference application performs local encrypted CRUD without identity.
- OIDC sign-in and Storage SQL work against the local backend.
- An approved Device can synchronize its selected encrypted vault to the Sync Server.
- Restart retains local and server state.
- Real-backend tests inspect remote ciphertext and prove that plaintext vault secrets are not stored there.

### M3 — Two-Device offline use and administration

- The shared Installation Device model replaces legacy User-owned Device transfer behavior.
- A second Device receives the selected vault through the supported synchronization path.
- Both Devices edit offline and converge after reconnection.
- Device revocation, deselection, and User/resource administration have explicit tested behavior.
- Restart and offline authorization use retained trusted state.

### M4 — Additional resource and client support

- Filesystem synchronization and Full/Passthrough residency complete their own end-to-end scenarios.
- KV and mesh packages receive supported, tested public contracts where needed.
- Browser and native support claims name their tested platforms.
- Packages are published only after their API, artifact, migration, and support policies are explicit.

M4 does not block M2 or M3.

## Non-goals for the first complete workflow

- A production-qualified password manager or broad password-manager feature set.
- Application-only data with no User namespace.
- Cross-User shares, folder ACLs, or Unix-style file grants.
- Immediate global revocation or guaranteed remote deletion.
- Durable replay, transparent multihop resource synchronization, or automatic bidirectional real-time delivery.
- Support for every resource type, browser, desktop platform, and npm package at once.
- Compatibility with every historical development format. Persisted-data and protocol compatibility require an explicit decision per change.

## Completion rule

A milestone is complete only when its user workflow passes through the public application and service contracts on a clean checkout. Unit tests, mounted routes, generated clients, and design documents alone do not complete a milestone.
