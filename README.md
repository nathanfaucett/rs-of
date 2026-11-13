# Offline-First IdP and Storage

[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)](LICENSE-MIT)
![Test Status](https://github.com/nathanfaucett/rs-of/actions/workflows/ci.yml/badge.svg)

An OAuth 2.0 and OpenID Connect identity provider with application-scoped, offline-first databases and filesystems.

Applications authenticate through OAuth/OIDC and manage resources in a `(user subject, application)` namespace. Data is stored locally, remains available offline, and synchronizes between approved devices when each peer's Device policy authorizes the selected resource.

## Services

- `idp-service` — OAuth/OIDC, users, clients, tokens, and signing keys.
- `management-service` — roles, permissions and assignments.
- `device-service` — Devices, endpoint bindings, approval/transfer/revocation, resource selection, administrator restrictions and sync policy. IdP owns Applications and token-signing trust.
- `storage-service` — provides authenticated database and filesystem resource APIs.
- `storage-server` — exposes storage over WebSocket.
- `file-system` — offline-first storage and synchronization.

## Storage

Storage is scoped by `(user subject, application_id)`. Each namespace can own multiple databases and filesystems with distinct stable IDs; display names need not be unique. OAuth clients in the same application share a user's resources. Storage-audience access tokens authorize Storage API actions such as `read` and `write`; they do not authorize peer synchronization. Device owns per-device whole-resource selection and administrator restrictions.

The Storage API provides create, list, get, and delete routes for databases and filesystems. Filesystem sockets use an explicit filesystem ID and a Storage-audience token.

## Offline-first filesystem

`file-system` stores data locally so reads and writes continue without network access.

`file-system` owns filesystem paths, metadata, content, persistence, and synchronization. Full devices retain metadata and content; Passthrough devices retain metadata only, read content from an available selected Full peer, and cannot write. Resource deletion is a tombstone; device deselection removes only that device's local copy.

## Devices

Each Node composes IdP, Management, Device and Storage, with Device-owned Device/endpoint authority and independent IdP token-signing capability. Separate owner processes/HTTPS bases create no extra Nodes or Devices. Device approval/pairing is Device-owned; endpoint possession is not resource permission, and an approved endpoint needs no individual IdP token signer. Joining uses a trusted reachable peer, not a Storage-only or privileged replica role.

The confirmed target allows each Node to mutate identity, policy, and resources against persistent trusted local state, including offline and after restart. No designated writer, central authorization service, promotion, or 30-second freshness lease is required. OAuth consumption is atomic locally, without a global single-use guarantee during partitions. See [system design](docs/design.md) and [operational qualification](docs/operations.md#qualification-boundary); this is not a claim of complete platform/deployment acceptance.

Approved devices synchronize explicitly selected resources peer-to-peer over authenticated Iroh streams. Each peer checks approved endpoint identity, both devices' selections, administrator allowance, and matching resource kind, ID, and namespace. OAuth tokens govern API requests, not mesh synchronization. Revocation blocks subsequent operations once learned locally; an already-authorized operation may finish. Temporary partition divergence is accepted, and revocation cannot erase data already copied offline.

`ofnet`/`iroh-chain` owns reusable Iroh endpoints, mesh/pairing mechanics, endpoint allowlist enforcement, and generic connections/routing. `of` owns domain identity, policy, catalogs, endpoint-to-Installation-Device trust, local authorization, and protocol-specific transport adapters wiring persistence owners. `ofdb` owns SQL/KV persistence, CRDT sync, staging, batching, and cleanup through public `ofdb-sql`/`ofdb-kv` only. `offs` owns filesystem persistence/sync, not transport. Generic tests belong to these owners; `of` tests authorization integration. Each Node shares one `iroh-chain` endpoint/router, with endpoint identity and locally trusted membership persisted and restored by `of`. Allowlisting is not resource authorization; pairing is not general access. Mesh messages and direct streams are distinct, with no transparent multihop database/filesystem sync claim. See [networking composition and interfaces](docs/design.md#networking-composition-and-trust).

## Local development backend

```sh
cargo run -p unified-server -- local --data-dir .data --port 1355 --origin http://localhost:5173 --no-tls
```

All four owners use one embedded database at `.data/unified.redb`:
`/idp`, `/device`, `/management`, and `/storage`. Each exposes `/health`.
No setup, admission code, CA, or mesh endpoint is required. APIs do not serve UI assets.
Omit `--no-tls` for HTTPS with a self-signed certificate and private key in `.data/tls/`.
The certificate and key are reused on restart. No OS trust is installed.
For local HTTPS clients, trust `.data/tls/cert.pem` explicitly.

The local administrator is `admin@local.test` / `password`. The local sign-in
client is `local-login`. Public PKCE clients are `app` (callback at the supplied
origin's `/callback`) and `password-manager` (same host and scheme, port 5174,
`/callback`). The issuer is the actual backend base plus `/idp`, including its
HTTP or HTTPS scheme. CORS permits the app and password-manager origins.

Optional additional users can be supplied with `--seed users.json`:

```json
{"users":[{"username":"test-user","password":"test-password"}]}
```

Seeding is repeatable. Conflicting retained credentials fail rather than being
replaced. Local mode uses public test credentials and reduced password-hash cost;
it binds loopback only. Do not expose it through a public proxy.

Keep the entire data directory for restart and backup. The owner entrypoints also
retain IdP private keys and Storage resource files. The database alone is not a
complete backup. This is not a migration for the previous unified-server layout.

**Validation is incomplete:** the owner HTTP constructors are blocked by existing
Device replication and filesystem snapshot compile errors. See
[ticket 04 validation](.scratch/backend-simplification/unified-validation.md).
The fresh-start, restart, HTTPS and OAuth HTTP tests are written, but have not passed.

## Independent Node setup

The separate `idp-server`, `device-server`, `storage-server`, and
`management-server` deployment paths retain their existing configuration and setup.
IdP, Device and Storage each use their own `/setup/owner/...` surfaces and protected
capability. Management has no fourth setup step. See [configuration](config/README.md)
and [operations](docs/operations.md). The replacement unified-server has no `serve`,
`create`, `join`, or YAML configuration commands.

## Applications

Applications use OAuth/OIDC for authentication and the generic storage API for data.

Web, desktop, and mobile clients belonging to the same application can share the same user data while each device remains independently usable offline.

Applications define their own file formats and domain models; device management, peer connections, and synchronization are handled by the underlying services.
