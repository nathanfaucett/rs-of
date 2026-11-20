# Target architecture

This document defines the target composition and ownership. Current support and qualification are in [README](../README.md#current-status).

## System model

An **Installation** contains Users, Applications, OAuth Clients, Devices, permissions, and Resources. A **Node** hosts services and retained local state. A **Sync Server** is a Node that exposes synchronization APIs.

A Resource belongs to one `(User subject, Application)` namespace. A Device is an approved Installation endpoint. A Selection requests a Resource replica on a Device. Devices do not own Resources and may serve several Users and Applications when policy permits.

## How the parts fit

```text
apps/app
  SvelteKit UI ---- OIDC and generated HTTPS clients ----┐
  Tauri native host -- local HTTPS and sync commands ----+--> services
                                                              |
                                  IdP + Management + Device + Storage
                                                              |
                                      Storage coordinates Resource access
                                                              |
                                   +--------------------------+-------------------+
                                   |                                              |
                                   v                                              v
                         db: SQL and KV Resources                     fs: filesystem Resources
                                   |                                              |
                                   +--------------------------+-------------------+
                                                              |
                                             authorized synchronization adapters
                                                              |
                                           net transports or HTTPS/WebSocket
                                                              |
                                                         peer Nodes
```

### Application: `apps/app`

The SvelteKit frontend uses OIDC and generated IdP, Management, Device, and Storage clients for setup, sign-in, administration, Resource discovery, and Selection. The Tauri host composes the native services, serves verified local HTTPS, and exposes native synchronization commands.

The application orchestrates these contracts. It does not replace service ownership or authorization. The password-manager application under `apps/password-manager` is the small encrypted-Resource reference consumer.

### Services: `services/`

- **IdP** authenticates Users and OAuth Clients and owns Applications, credentials, grants, and token-signing trust.
- **Management** owns roles, permissions, and assignments and authorizes administration.
- **Device** owns Device records, endpoint identity binding, approval, revocation, and configuration.
- **Storage** owns Resource catalogs, namespaces, Resource access, Selections, and synchronization adapters.

Their HTTP contracts live in `openapi/`. `unified-server` composes the services for local development; composition does not merge their ownership.

### Database: `db/`

`ofdb-sql` and `ofdb-kv` provide local persistence, queries, explicit transactions, history, and CRDT reconciliation. Services consume only these public facades:

- **Database** owns storage lifecycle, hosting, and synchronization.
- **Client** owns queries and explicit transactions.

Storage engines and implementation types stay behind this boundary. `ofdb` does not own Users, Devices, Resource authorization, peer discovery, or transport policy. Implementation invariants live in [database design](../db/design.md).

### Filesystem: `fs/`

`offs` owns paths, file metadata, content, persistence, history, and reconciliation. Storage maps filesystems into Resource namespaces and enforces access. Full/Passthrough Residency controls local content retention; it is not Resource ownership or Selection.

### Networking: `net/`

`ofnet` provides Iroh endpoints, pairing, allowlists, mesh messages, and byte streams. Storage adapters bind authorized SQL, KV, and filesystem synchronization to those transports. Pairing and allowlisting do not grant Device approval, Resource access, or Selection.

## Context ownership

| Context | Owns | Does not own |
| --- | --- | --- |
| IdP | Identity, credentials, Applications, OAuth Clients, grants, signing trust | Device records, Resource catalogs, Management assignments |
| Management | Roles, permissions, assignments, administrative authorization | Device or User lifecycle, Resource content |
| Device | Endpoint binding, approval/revocation, Device configuration | Resources and role definitions |
| Storage | Resource catalogs, namespaces, access, Selections, Resource synchronization | OAuth identity, role assignments, endpoint pairing mechanics |

## Identity, Device access, and Selection

Each operation keeps three checks separate:

1. **Identity:** which Principal made the request?
2. **Resource access:** may that Principal act in this User/Application namespace?
3. **Replica placement:** is the Device approved, and is the Resource selected for it?

An OIDC token does not prove Device approval. Endpoint pairing does not grant Resource access. Network allowlisting is not Selection. Device revocation blocks later authorized use when learned; it does not transfer Resource ownership or erase copies already delivered.

## Networking and synchronization

A native Node may share one `ofnet` endpoint and router. `ofnet` supplies transport mechanics; services supply authenticated Installation/Device binding and protocol-specific Resource adapters. HTTPS and WebSocket paths follow the same separation.

Local commit notifications are not durable remote replay. Applications must provide authorized transport and remote delivery or explicit reconciliation. Earlier committed progress can survive a later failed session. Mesh forwarding does not provide transparent multihop SQL, KV, or filesystem synchronization.

## Source ownership

`nathanfaucett/rs-of` is canonical. The `db/`, `fs/`, and `net/` trees in this repository are their sources of truth. Do not manually mirror changes into sibling repositories.
