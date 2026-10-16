# Offline-First IdP and Storage

[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)](LICENSE-MIT)
![Test Status](https://github.com/nathanfaucett/rs-of/actions/workflows/ci.yml/badge.svg)

An OAuth 2.0 and OpenID Connect identity provider with application-scoped, offline-first databases and filesystems.

Applications authenticate through OAuth/OIDC and manage resources in a `(user subject, application)` namespace. Data is stored locally, remains available offline, and synchronizes between approved devices when each peer's Management policy authorizes the selected resource.

## Services

- `idp-service` — OAuth/OIDC, users, clients, tokens, and signing keys.
- `management-service` — roles, permissions, resource selection, administrator restrictions, and sync policy. IdP owns Applications and Device lifecycle.
- `storage-service` — provides authenticated database and filesystem resource APIs.
- `storage-server` — exposes storage over WebSocket.
- `file-system` — offline-first storage and synchronization.

## Storage

Storage is scoped by `(user subject, application_id)`. Each namespace can own multiple databases and filesystems with distinct stable IDs; display names need not be unique. OAuth clients in the same application share a user's resources. Storage-audience access tokens authorize Storage API actions such as `read` and `write`; they do not authorize peer synchronization. Management owns per-device whole-resource selection and administrator restrictions.

The Storage API provides create, list, get, and delete routes for databases and filesystems. Filesystem sockets use an explicit filesystem ID and a Storage-audience token.

## Offline-first filesystem

`file-system` stores data locally so reads and writes continue without network access.

`file-system` owns filesystem paths, metadata, content, persistence, and synchronization. Full devices retain metadata and content; Passthrough devices retain metadata only, read content from an available selected Full peer, and cannot write. Resource deletion is a tombstone; device deselection removes only that device's local copy.

## Devices

Every Node hosts IdP, Management, and Storage, with a persistent Device identity and an independent local private signer. Devices enroll and pair through IdP with locally checked Management permissions. Joining uses a trusted reachable peer, not a Storage-only or privileged replica role.

The confirmed target allows each Node to mutate identity, policy, and resources against persistent trusted local state, including offline and after restart. No designated writer, central authorization service, promotion, or 30-second freshness lease is required. OAuth consumption is atomic locally, without a global single-use guarantee during partitions. See [system design](docs/design.md) and [operational qualification](docs/operations.md#qualification-boundary); this is not a claim of complete platform/deployment acceptance.

Approved devices synchronize explicitly selected resources peer-to-peer over authenticated Iroh streams. Each peer checks approved endpoint identity, both devices' selections, administrator allowance, and matching resource kind, ID, and namespace. OAuth tokens govern API requests, not mesh synchronization. Revocation blocks subsequent operations once learned locally; an already-authorized operation may finish. Temporary partition divergence is accepted, and revocation cannot erase data already copied offline.

`ofnet`/`iroh-chain` owns reusable Iroh endpoints, mesh/pairing mechanics, endpoint allowlist enforcement, and generic connections/routing. `of` owns domain identity, policy, catalogs, endpoint-to-Installation-Device trust, local authorization, and protocol-specific transport adapters wiring persistence owners. `ofdb` owns SQL/KV persistence, CRDT sync, staging, batching, and cleanup through public `ofdb-sql`/`ofdb-kv` only. `offs` owns filesystem persistence/sync, not transport. Generic tests belong to these owners; `of` tests authorization integration. Each Node shares one `iroh-chain` endpoint/router, with endpoint identity and locally trusted membership persisted and restored by `of`. Allowlisting is not resource authorization; pairing is not general access. Mesh messages and direct streams are distinct, with no transparent multihop database/filesystem sync claim. See [networking composition and interfaces](docs/design.md#networking-composition-and-trust).

## Automatic Node bootstrap

With the `unified-server` binary built with its `cli` feature, ordinary setup needs no config file, internal IDs, relationship secrets, audiences, manual context roots or password file:

```sh
unified-server create
unified-server serve
# On another machine, with source acceptance enabled:
unified-server join
unified-server serve
```

`create` defaults to administrator `operator` and Node name `Local Node`, with hidden password entry and confirmation. `--password-file` is optional automation. `join` prompts for a trusted peer endpoint fingerprint (or accepts `--peer`); verify it through a trusted channel. The source's ordinary authorized session enables acceptance and approves the exact offer through `/idp/node-joins` routes. Authenticated issuer discovery is not automatic enrollment.

Both create and join automatically retain a public operator-login Client with no Client secret. CLI output gives its actual ID, login scopes and token-exchange audiences; `serve` also prints the local token endpoint/API addresses. Use normal password authentication at `POST /idp/oauth2/token` with that Client ID and an existing User's username/password, then ordinary token exchange for authorized owner calls. No manual Client registration is needed; keep passwords out of argv/logs.

The host derives and retains settings, generates four independent service credentials through the secure provider, and retains a unique logical Canonical Issuer separate from its local HTTPS API address. Join retains the authenticated Installation issuer. Both create and join require OS CA installation, elevation/trust consent and production HTTPS verification; there is no plaintext/browser-only or skip-install fallback. Setup exits after completion; `serve` reopens existing state only. `--data-dir` and explicit `--config` are optional alternatives, not prerequisites.

See [Node operations](docs/operations.md#automatic-unified-cli-bootstrap) and the [bootstrap reference](config/unified/README.md) for username/password examples, approval, retry, retention and qualification limits. Desktop fresh create also derives a unique retained issuer and join uses the shared authenticated discovery helper; manual issuer UI/input is removed. The current CLI is Unix-only; documented code paths and historical Ubuntu results are not new native installation or universal platform acceptance claims.

## Applications

Applications use OAuth/OIDC for authentication and the generic storage API for data.

Web, desktop, and mobile clients belonging to the same application can share the same user data while each device remains independently usable offline.

Applications define their own file formats and domain models; device management, peer connections, and synchronization are handled by the underlying services.
