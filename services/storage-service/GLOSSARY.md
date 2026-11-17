# Storage

The resource-management and database/filesystem synchronization context. Shared terms are defined in [the root glossary](../../GLOSSARY.md). Resource isolation and synchronization policy are described in [Storage design](docs/design.md).

## Language

**Storage Resource**:
A database or filesystem in one User's Application namespace, with a distinct stable identity and a potentially non-unique display name. Multiple OAuth Clients of that Application share the namespace, while other subjects cannot open it.

**Resource Catalog**:
The synchronized record of Storage Resource identity, ownership, display names, and deletion status. Discovery does not imply local possession of content or permission to access it.

**Storage-Audience Access Token**:
A standard IdP access token for the Storage API, obtained through sign-in or exchange from an Application client's user token. It binds subject, Application, and API action limits, not mesh synchronization; it is neither a separate token type nor a single-use storage session.
_Avoid_: Storage Session, storage client token

**Resource Ownership**:
The exclusive User/Application namespace boundary for a database or filesystem and every file in it. Other subjects and Applications cannot access or select it; there are no file permission bits, groups, or cross-user grants.
_Avoid_: Device grant, endpoint grant

**Filesystem ID**:
The stable identity of one filesystem instance, independent of its local root path. It scopes filesystem authorization and synchronization.
_Avoid_: Vault ID

**Database ID**:
The stable identity of one Application database, separate from IdP/Management state and every Filesystem ID.

**Residency**:
A Device-local choice for a filesystem resource or path: Full stores metadata and content locally, while Passthrough stores metadata and reads content from an authorized available Full peer but cannot write. Residency is not synchronized.

**File System**:
A local-first replicated storage engine for paths, metadata, and file content, separate from Application databases and IdP/Management records. It has no file permission bits, groups, or cross-user grants.

**File Entry**:
Metadata for one filesystem path, with a stable file identity, content revision, and provider information. Deleted entries are represented by synchronized Tombstones.

**Tombstone**:
A synchronized deletion record that prevents a deleted File Entry or Storage Resource from returning when an offline Device reconnects.

## Synchronization

**Sync Transport**:
The WebSocket-based binary protocol (wss://<storage-base>/sync/ws) carrying ofdb SQL v7 owner messages. Distinct from Storage REST API and filesystem DATA sockets. Uses custom framing (version/flags/length/CRC32/Postcard) and Hello/Attach handshake.

**Sync Session**:
A single attached WebSocket connection with negotiated direction (Push/Pull/Bidirectional), frame/session byte limits, and backend replica identity. Created via Hello -> AttachRequest -> AttachResponse exchange with a storage:sync audience token.

**Store Identity**:
The persistent identity of a local database participating in sync: storeId (database UUID), replicaId (browser/device instance UUID), creationEpoch (database creation timestamp), schemaFingerprint (32-byte hash of schema). Must be stable across sessions; do not generate new replicaId per attach.

**Sync Direction**:
Push (local -> remote), Pull (remote -> local), Bidirectional (both). The server currently always grants Bidirectional regardless of request; direction enforcement is not yet implemented in run_sync_session.

**Sync Attachment**:
The handshake phase (Hello, AttachRequest with token, AttachResponse). Succeeds before database open; a valid attach can be followed by immediate close if policy/database checks fail.

**Owner Sync Engine**:
The ofdb-sql synchronization logic (Database::synchronize_checked, synchronize_with_stage_factory_checked) that implements the SQL merge/apply/acknowledgement contract. Runs in-process; not provided by this crate. The transport (SyncClient, WebSocketTransport) only carries opaque owner messages.

**Sync Transport Trait**:
ofdb_sql::SyncTransport -- send(Vec<u8>) / receive() -> Vec<u8>. Implemented by WebSocketTransport (server) and BrowserSyncTransport (Rust WASM client). The npm @ofsdk/storage-sync-client provides a raw byte pipe (SyncSession.send/receive) but does NOT implement this trait -- an adapter is required.

**Browser Sync Integration (open)**:
The work to integrate a browser SQL owner engine with the sync transport. Requires: (1) WASM ofdb-sql binding, (2) SyncTransport adapter from SyncSession to ofdb_sql::SyncTransport, (3) storage:sync token acquisition via IdP token exchange, (4) stable StoreIdentity derivation from local database. No browser app has completed this integration.

---

Full sync architecture: [crates/storage-service/docs/sync-architecture.md](crates/storage-service/docs/sync-architecture.md)
