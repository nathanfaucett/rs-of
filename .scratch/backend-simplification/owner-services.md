# Local owner entrypoints

Ticket: [03](issues/03-refactor-owner-services.md).

All four public entrypoints are exported from their server crate:

| Crate | Function | Routes |
| --- | --- | --- |
| `idp-server` | `build_local_router(data_dir, port, origin, db).await` | `/idp` |
| `device-server` | `build_local_router(data_dir, port, idp_base, db).await` | `/device` |
| `management-server` | `build_local_router(data_dir, port, idp_base, storage_base, db).await` | `/management` |
| `storage-server` | `build_local_runtime(data_dir, port, idp_base, device_base, db)` | `/storage` |

Each accepts `Option<Arc<dyn of_model::db::Database>>`. `Some` uses that database's query client without opening an owner database or falling back to a private file. Initialize its unified schema first, for example with `EmbeddedDatabase::open` or `MemoryDatabase::new`. `None` opens `{owner}.redb` in `data_dir` and initializes the unified schema there. The shared client retains the underlying engine after the trait-object handle is dropped.

The canonical local IdP issuer is `http://127.0.0.1:{port}/idp`. Pass this issuer as `idp_base` to the other owners. Backend URLs use `127.0.0.1`; frontend callback host and scheme come from `origin`.

These constructors do not run setup, admission, capability operations, TLS provisioning, CA generation, or mesh startup. They return the normal owner routers. Storage has no data protocol handler or replication task in this mode. Existing independent `serve` entrypoints, configuration, and setup paths are retained. The setup crate is unchanged.

## Local identity

- Administrator username and verified email: `admin@local.test`; password: `password`.
- Explicit initial administrator grants come from `management_service::INITIAL_ADMINISTRATOR_ACTIONS`.
- Public PKCE clients: `app` and `password-manager`, with authorization-code, refresh-token, and token-exchange grants.
- App callback: `origin` with path `/callback`.
- Password-manager callback: the same frontend host and scheme, port `5174`, path `/callback`.
- Confidential local service clients: `local-device`, `local-management`, and `local-storage`, secret `local-password`. They can request only the existing IdP validation and user-lookup scopes.
- Stable user, credential, role, permission, member, and signer identities make repeated seed calls safe. Existing client registrations are not recreated. Conflicting administrator credentials or signer trust fail instead of being replaced.
- The local signer uses the normal signer trust repository, without a setup operation. Private keys are stored in owner-private files under `idp-keys/`, not the OS credential store. The key namespace uses the canonical data directory. This is local signer trust, not approved Device membership or a live transport endpoint.

**Local development only.** The seeded credentials are public. Do not expose this mode to an untrusted network. Unix directory/file permissions are checked for the local IdP key store; Windows ACL behavior is not qualified.

## Storage sharing and files

The local Storage database runtime uses the injected SQL client for the actual database catalog. The existing `namespaces`, `resources`, and `databases` tables store namespace identity, resource names, tombstones, and initialization state. Create, ensure/reserve, finish, lookup, list, selected-resource registration, deletion, and owner-cleanup inventory all use this catalog. Read-modify-write operations use one transaction; a colliding resource in another namespace is rejected. There is no parallel JSON database catalog in local mode.

Resource contents still need files:

- Each logical SQL database has its own redb content file. Its KV sidecar remains separate. These are user resources, not owner catalog files.
- Physical local-copy journals remain files. They record local cleanup and content initialization, not shared logical identity.
- `storage-local.peer` retains the filesystem's local peer label across restarts. It is public identity data, not a transport private key. No endpoint is bound.
- The offs filesystem API owns KV-backed filesystem catalogs, per-filesystem metadata, residency, and content files. It has no SQL-client injection boundary. Those files remain under `filesystems/`; this ticket does not claim that filesystem catalogs use the unified SQL tables.

Keep the resource directories, journals, peer label, and IdP private keys when restarting. `unified.redb` alone is not a complete backup. Filesystem SQL integration would need an offs catalog interface change, not an unused SQL metadata mirror.

The database catalog API is synchronous. Its SQL transactions run on scoped worker threads so it can be called from a Tokio runtime without nesting that runtime. Production HTTP constructors and live network-database behavior remain unqualified while base dependency errors prevent the owner suites from compiling.

## Validation boundary

Normal constructor tests are in each owner's `tests/local.rs`. The [isolated validation crate](owner-validation/README.md) tests the actual seed and database catalog/runtime modules without the broken Device replication and offs dependencies. Its pass does not establish that owner HTTP constructors compile or that independent deployments run. See the ticket for exact results and remaining acceptance gaps.
