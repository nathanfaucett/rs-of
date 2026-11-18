# Target architecture

This is the accepted target architecture. Implementation is still migrating from User-owned Devices and shared-host database adapters. Current support and qualification are recorded in [README](../README.md#current-status), not inferred from this document.

## System model

An **Installation** contains Users, Applications, OAuth Clients, Devices, permissions, and Resources. A **Node** hosts service contexts and their retained local state. A Node exposed as a synchronization backend is called a **Sync Server** in operational documentation.

```text
Installation
├── Users
├── Applications
│   └── OAuth Clients
├── Devices
│   └── Selections ───────┐
└── Resources             │
    └── Replicas ─────────┘
```

A Resource belongs to one `(User subject, Application)` namespace. A Device is an approved endpoint that can retain Resource replicas. A Selection is the desired relationship between one Resource and one Device.

Devices do not own Resources. One Device may serve several Users and Applications when policy permits it.

## Context ownership

| Context | Owns | Does not own |
| --- | --- | --- |
| IdP | Users, credentials, Applications, OAuth Clients, grants, token-signing trust | Device records, Resource catalogs, Management assignments |
| Management | Roles, permissions, assignments, authorization decisions | Device or User lifecycle, Resource content |
| Device | Endpoint identity binding, Device approval/revocation, Device configuration | User/Application Resources, role definitions |
| Storage | Resource catalogs, namespaces, Resource access, replicas, Storage API execution | OAuth identity, role assignments, endpoint pairing mechanics |
| `ofdb` | SQL/KV persistence, transactions, history, CRDT reconciliation, sync execution | Users, Devices, domain authorization, peer discovery |
| `offs` | Filesystem paths, metadata, content, persistence, reconciliation | Installation policy, generic network routing |
| `ofnet` | Iroh endpoints, pairing, allowlists, mesh messages, byte streams | Device approval, Resource authorization, Selection |

Device service owns Device records. Management decides whether a Principal may administer a Device or Resource. Storage checks Resource access. These checks are related but separate.

## Identity, Device access, and Selection

Three questions must remain separate:

1. **Identity:** which User, OAuth Client, or service Principal made this request?
2. **Resource access:** may that Principal perform this action in the User/Application namespace?
3. **Replica placement:** is the Device approved, and is the Resource selected for it?

A valid OIDC token does not prove Device approval. Endpoint pairing does not grant Resource access. A catalog entry does not prove local possession or permission. Network allowlisting is not Selection.

The target model has no User-to-Device ownership transfer. Device revocation stops later use when learned; it does not change Resource ownership or erase copies already delivered. User deletion and Resource deletion are separate policies.

## Reference request flow

```text
Password-manager reference app
  → local encrypted SQL operations without an account
  → optional OIDC sign-in at a Sync Server
  → verified User/Application identity
  → Storage token exchange and Resource authorization
  → approved Device plus Resource Selection for synchronization
  → encrypted Resource replica on the selected Device
```

The application encrypts vault records before Storage receives them. OIDC protects API access; it does not encrypt or decrypt vault records.

## Database boundary

Application services consume only `ofdb-sql` and `ofdb-kv`:

- **Database** owns open/close, hosting, and synchronization lifecycle.
- **Client** owns queries and explicit transactions.

Redb, Automerge, B-tree, shared-host, and engine implementation types do not cross this boundary. One server process does not require one physical database file. Persistence sharing must not require services to import internal engine crates.

See [ADR 0003](adr/0003-ofdb-facade-boundary.md). SQL/KV implementation details live in [database design](../db/design.md).

## Networking and synchronization

Each native Node may share one `ofnet` endpoint/router. `ofnet` supplies transport mechanics; `of` supplies authenticated Installation/Device binding and protocol-specific Resource adapters.

Local commit notifications are not durable remote replay. Callers must provide authorized transport and remote delivery or explicit reconciliation. Earlier valid committed progress can survive a later failed sync session. Neither global atomicity nor immediate global revocation is promised.

Mesh Broadcast/Direct Message and direct Byte Streams are different capabilities. Mesh forwarding does not imply transparent multihop SQL, KV, or filesystem synchronization.

## Deployment composition

- **Local unified:** four owner routers in one loopback development process with seeded test identities. It is not a production deployment mode.
- **Independent owners:** separate IdP, Management, Device, and Storage processes with retained setup paths. Current qualification is limited.
- **Desktop:** native composition and OS trust integration. Current GUI/platform acceptance is limited.

Process composition does not change domain ownership. See [deployment boundaries](backend-deployments.md).

## Source ownership

`nathanfaucett/rs-of` is canonical. The `db/`, `fs/`, and `net/` trees in this repository are the sources of truth. Other repositories may be generated mirrors or publication targets, but developers must not manually mirror changes.

See [ADR 0001](adr/0001-canonical-monorepo.md).
