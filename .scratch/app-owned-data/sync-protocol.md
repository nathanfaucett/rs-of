# App-Owned Data Sync Protocol

Status: draft

## Overview

This document defines a backend-neutral synchronization protocol for app-owned data stores. The protocol carries existing ofdb SQL/KV and offs filesystem owner protocols rather than replacing them with remote CRUD operations. It is designed for browser-capable transport (WebSocket over TLS) and must be proven with two independent persistent app stores and a test backend before any service conversion.

## 1. Version Negotiation

### 1.1 Protocol Versioning

Each sync session begins with a `Hello` message exchange:

```
Hello {
    protocol_version: u16,
    manifest: SyncManifest,
    store_identity: StoreIdentity,
    supported_protocols: ProtocolId[],
}
```

- `protocol_version`: Monotonically increasing protocol version (starts at 1)
- `manifest`: Current store manifest (schema, CRDT state, etc.)
- `store_identity`: Stable identity of the local store/replica
- `supported_protocols`: List of owner protocols this peer can speak (e.g., "ofdb-sql-v7", "offs-fs-v1")

### 1.2 Negotiation Rules

1. Initiator sends `Hello` with highest supported `protocol_version`
2. Responder replies with `Hello` using `min(initiator.version, responder.max_version)`
3. Both peers must agree on at least one `ProtocolId` from `supported_protocols`
4. If no compatible protocol, responder sends `Abort("incompatible protocol")`
5. Version mismatch beyond negotiated version → `Abort("version mismatch")`

### 1.3 Protocol IDs

| Protocol ID | Owner | Carried Protocol |
|-------------|-------|------------------|
| `ofdb-sql-v7` | ofdb | sql-sync PROTOCOL_VERSION=7 |
| `ofdb-kv-v1` | ofdb | kv-sync (TBD) |
| `offs-fs-v1` | offs | filesystem sync v1 |

New protocols are additive; existing versions are never modified.

## 2. Stable Store/Replica Identity

### 2.1 StoreIdentity

```
StoreIdentity {
    store_id: Uuid,           // Stable across restarts, unique per vault
    replica_id: Uuid,         // Unique per device/browser profile
    creation_epoch: u64,      // Monotonic counter incremented on re-key
    schema_fingerprint: Hash, // Schema + vault format hash
}
```

### 2.2 Identity Rules

1. `store_id` is generated once at vault creation and never changes
2. `replica_id` is generated per device/profile; persists in local storage
3. `creation_epoch` increments only on explicit vault re-key (not on sync)
4. `schema_fingerprint` changes only on schema migration
5. Backend must store and verify `(store_id, replica_id)` pairs
6. **Account/backend changes must not re-key, replace, or merge unrelated vaults**

### 2.3 Explicit Attachment

```
AttachRequest {
    store_identity: StoreIdentity,
    auth_token: AuthToken,      // Proves authorization for this store
    direction: SyncDirection,   // Push, Pull, or Bidirectional
}

AttachResponse {
    accepted: bool,
    backend_replica_id: Uuid,   // Backend's identifier for this replica
    session_id: Uuid,
    granted_direction: SyncDirection,
    max_frame_bytes: u32,
    max_session_bytes: u64,
}
```

- Attachment is explicit; no implicit binding from account login
- Each store requires separate attachment
- Backend rejects attachment for unknown/unauthorized `(store_id, replica_id)`
- Direction is per-session; can differ from granted direction

## 3. Authenticated Authorization

### 3.1 Authorization Model

- **Local app labels are not credentials** — they are metadata only
- Backend authenticates via `AuthToken` (opaque, backend-defined)
- Authorization is per-store, per-replica, per-direction
- Enrollment: first attachment requires backend-specific enrollment flow
- Learned revocation: backend can revoke `AuthToken` at any time
- Subsequent operations must re-verify authorization on reconnect

### 3.2 AuthToken Requirements

- Opaque to the app; backend defines format (JWT, Macaroon, opaque string)
- Must bind to `(store_id, replica_id, direction)`
- Must support revocation without requiring browser Node/IdP
- App obtains token via backend-specific setup (out of band for this protocol)

### 3.3 Authorization Checks

1. On `AttachRequest`: verify token validity, store/replica match, direction grant
2. On reconnect: re-verify token; revoked tokens → `Abort("authorization revoked")`
3. On direction change: new `AttachRequest` required
4. Failed auth stops exchange; **does not reset local data or prevent local editing**

## 4. Browser Transport

### 4.1 Transport Requirements

- **WebSocket over TLS (wss://)** as the primary browser-capable transport
- Reliable, ordered, framed byte stream
- Must work in browser without native host, Iroh, or custom protocols
- Existing Iroh adapters remain for native targets; not a browser requirement

### 4.2 Frame Format

```
Frame {
    version: u8,        // Protocol framing version (1)
    flags: u8,          // Bit flags (see below)
    length: u32,        // Payload length (max 16 MiB)
    payload: bytes,     // Postcard-encoded SyncMessage
    crc32: u32,         // CRC32C of payload
}
```

### 4.3 Frame Flags

| Bit | Name | Meaning |
|-----|------|---------|
| 0 | `CONTINUATION` | Payload continues in next frame |
| 1 | `COMPRESSED` | Payload is zstd-compressed |
| 2 | `PRIORITY` | High-priority (ack, abort, small control) |
| 3-7 | Reserved | Must be 0 |

### 4.4 Limits

| Limit | Value | Rationale |
|-------|-------|-----------|
| Max frame payload | 16 MiB | Browser memory, prevents OOM |
| Max session bytes | 1 GiB | Bounded work per session |
| Max message size | 1 MiB | Individual SyncMessage limit |
| Frame timeout | 30 s | Detect stalled connections |
| Session timeout | 10 min | Prevent indefinite sessions |

### 4.5 Backpressure

- Receiver advertises `max_frame_bytes` and `max_session_bytes` in `AttachResponse`
- Sender must not exceed advertised limits
- Receiver may send `Abort("frame too large")` or `Abort("session too large")`
- Application-level flow control via `SyncSelection` (what to sync)

## 5. Framing and Message Exchange

### 5.1 Message Types (Carried in Frames)

All messages are postcard-encoded `SyncMessage` enums. The protocol carries **owner protocol messages** opaquely:

```
SyncMessage {
    Hello(Hello),
    Attach(AttachRequest),
    AttachResponse(AttachResponse),
    OwnerMessage(OwnerProtocolMessage),  // Opaque bytes for ofdb-sql-v7, etc.
    OwnerMessageBatch(Vec<OwnerProtocolMessage>),
    Ack(Acknowledgement),
    Progress(ProgressReport),
    Cancel(CancelReason),
    Reconnect(ReconnectToken),
    Abort(AbortReason),
    Ping,
    Pong,
}
```

### 5.2 Owner Protocol Encapsulation

- Each `ProtocolId` defines its own message types (e.g., `ofdb-sql-v7` uses `sql-sync::SyncMessage`)
- Sync protocol does **not** decode owner messages; treats as opaque `Vec<u8>`
- Framing/limits apply to encapsulated messages
- Multiple owner protocols can be multiplexed over one session (future)

### 5.3 Session Flow

```
1. TCP/TLS connect → WebSocket upgrade
2. Initiator → Hello
3. Responder → Hello (negotiated version)
4. Initiator → AttachRequest
5. Responder → AttachResponse
6. [Owner protocol exchange begins]
7. ... OwnerMessage / OwnerMessageBatch / Ack / Progress ...
8. Either peer → Cancel / Abort / Done
9. Clean WebSocket close
```

## 6. Cancellation

### 6.1 Cancel Message

```
Cancel {
    reason: CancelReason,
    session_bytes_sent: u64,
    session_bytes_received: u64,
}

CancelReason {
    UserRequested,
    BackpressureExceeded,
    LocalError(String),
    SwitchingBackend,
    AppBackgrounded,
}
```

### 6.2 Cancellation Semantics

- **Cancellation is not proof of rollback** — sender may have persisted writes
- On cancel: stop sending, process in-flight, send `Ack` for completed work
- Receiver acknowledges cancel with `Ack { acknowledged_bytes: u64 }`
- Partial progress is durable; next session resumes from acknowledged state
- Local data remains usable; no reset on cancel

## 7. Partial Progress and Acknowledgements

### 7.1 Progress Reporting

```
ProgressReport {
    phase: SyncPhase,
    bytes_sent: u64,
    bytes_received: u64,
    units_completed: u64,
    units_total: u64,        // Estimated, may be unknown
    eta_ms: Option<u64>,
}

SyncPhase {
    Negotiating,
    Attaching,
    ExchangingManifests,
    ExchangingInventories,
    SendingSnapshots,
    ReceivingSnapshots,
    SendingChanges,
    ReceivingChanges,
    ApplyingChanges,
    Finalizing,
}
```

### 7.2 Acknowledgements

```
Acknowledgement {
    acknowledged_bytes: u64,     // Durably applied on receiver
    acknowledged_units: Vec<SyncUnitId>,  // Protocol-specific unit IDs
    receiver_state_hash: Hash,   // For reconciliation verification
}
```

### 7.3 Acknowledgement Rules

1. **Local save ≠ sync acknowledged** — states remain distinct through failures
2. `Ack` proves **one backend** received and applied; not all-device durability
3. Sender retains unacknowledged work for retry
4. Receiver sends `Ack` periodically and on phase boundaries
5. `receiver_state_hash` enables detection of silent corruption/divergence

## 8. Reconnect and State Reconciliation

### 8.1 Reconnect Token

```
ReconnectToken {
    session_id: Uuid,
    last_acknowledged_bytes: u64,
    last_acknowledged_units: Vec<SyncUnitId>,
    sender_state_hash: Hash,
    receiver_state_hash: Hash,
    timestamp: u64,  // Unix ms
}
```

### 8.2 Reconnect Flow

1. On disconnect, both peers retain `ReconnectToken`
2. On reconnect: new WebSocket → `Hello` → `AttachRequest` with `reconnect_token`
3. Backend verifies token, returns `AttachResponse` with `resume_from_bytes`
4. Peers exchange manifests/inventories from resume point
5. Idempotent reconciliation: duplicate units detected via `SyncUnitId`
6. **No silent reset** on stale/missing/denied/corrupt backend state

### 8.3 Idempotency

- All owner protocol operations must be idempotent by `SyncUnitId`
- `SyncUnitId` includes: `(table, row, change_id)` for SQL; `(path, version)` for FS
- Duplicate application is safe; receiver acknowledges but does not re-apply

## 9. Conflicts and Tombstones

### 9.1 Conflict Definition

A conflict occurs when:
- Same logical record modified on both peers since last sync
- Encrypted records: ciphertext differs → **conflict preserved, not merged**
- Schema incompatibility: `schema_fingerprint` mismatch → `Abort`

### 9.2 Conflict Handling

```
ConflictReport {
    store_id: Uuid,
    replica_id: Uuid,
    protocol_id: ProtocolId,
    conflicts: Vec<ConflictRecord>,
}

ConflictRecord {
    unit_id: SyncUnitId,
    local_version: OwnerVersion,
    remote_version: OwnerVersion,
    local_payload: bytes,   // Opaque, encrypted for password manager
    remote_payload: bytes,  // Opaque
}
```

### 9.3 Conflict Rules

1. **Encrypted credential edits remain recoverable, not silently merged or discarded**
2. Conflicts reported to app via `ConflictReport`; app decides resolution
3. App resolves by writing new version (creates new `SyncChangeId`)
4. Tombstones: explicit deletion markers with `SyncChangeId`
5. **Deletions do not resurrect from stale peers** — tombstone wins if newer
6. Tombstone GC: backend retains for `tombstone_retention_days` (configurable, default 30)

### 9.4 Owner Protocol Conflict Semantics

- **ofdb-sql-v7**: Uses CRDT history; conflicts = concurrent changes to same row
- **offs-fs-v1**: Uses file metadata + content hashes; conflicts = concurrent edits
- Protocol carries owner conflict resolution; sync protocol only transports

## 10. Metadata Leakage

### 10.1 Exposed Metadata

The backend observes:
- `store_id`, `replica_id` (pseudonymous, not linked to identity without token)
- Sync timing, frequency, byte counts
- `schema_fingerprint` (schema version, not content)
- Conflict occurrence (not content)
- Tombstone presence (not deleted content)

### 10.2 Protected Metadata

Backend **must not** observe:
- Plaintext secrets or unlock keys
- Record content (encrypted payloads are opaque)
- `AuthToken` binding to real identity (backend knows token, not necessarily user)
- Cross-store correlation (separate stores = separate sessions)

### 10.3 Rollback/Replay Limits

- Backend must document: max rollback window, replay protection
- Protocol includes `receiver_state_hash` in `Ack` for detection
- App can verify backend state matches acknowledged state

## 11. Key Distribution and Recovery

### 11.1 Key Separation

- **Local unlock key**: Derived from user password; never leaves device
- **Sync encryption key**: Separate key for encrypting sync payloads (if E2EE)
- **Backend auth key**: `AuthToken` for authorization; not a decryption key
- **Account access ≠ vault decryption** — separate credentials

### 11.2 Key Distribution

- Sync payload encryption (if used): keys exchanged out-of-band or via PAKE
- Protocol does not mandate E2EE; TLS provides transport encryption
- If E2EE: each replica has pairwise keys; backend cannot decrypt
- Key rotation: new keys negotiated via `AttachRequest` with new `creation_epoch`

### 11.3 Recovery

- **Encrypted export**: App exports vault + `store_id` + keys (user-controlled)
- **Verified restore**: Import validates `schema_fingerprint`, `store_id`, decrypts
- **No backend recovery service** — backend has no plaintext keys
- Lost `store_id`/keys = lost vault; no "forgot password" via backend

## 12. Test Harness Requirements

### 12.1 Two-Store Proof

The test harness must demonstrate:

1. **Two independent persistent app stores** (different `store_id`, `replica_id`)
2. **One test backend** implementing the protocol
3. **Real browser transport** (WebSocket over TLS, localhost for testing)
4. **Actual protocol exchange** — not mocked, not snapshot copy

### 12.2 Test Scenarios

| Scenario | Description |
|----------|-------------|
| Offline edits | Both stores edit offline; sync converges |
| Interrupted transfer | Kill network mid-sync; reconnect completes |
| Repeated exchange | Multiple sync cycles converge |
| Conflicts | Concurrent encrypted edits → conflict reported |
| Deletions | Delete on one, edit on other → tombstone wins |
| Denied access | Invalid token → exchange stops, local works |
| Malformed frames | Oversized/corrupt frames → abort, no data loss |
| Version mismatch | Incompatible protocol → clean abort |
| Revocation | Token revoked mid-sync → stop, local works |
| Backend change | Re-attach to new backend → explicit, no re-key |

### 12.3 Harness Implementation

- Test backend: minimal Rust service using `sql-sync` engine
- Browser test: Playwright/Puppeteer driving two browser contexts
- Persistent stores: IndexedDB-backed `ofdb-sql` in each context
- Transport: `ws://localhost:PORT` (TLS in CI via self-signed cert)
- **Not a production service** — evidence for service plan only

### 12.4 KV/Filesystem Claims

- SQL protocol proven first
- KV/filesystem compatibility requires separate binding/protocol tests
- Do not claim KV/FS support until their own harnesses pass

## 13. JS/WASM Binding Extensions

### 13.1 Missing SQL Owner Sync Exposure

Current `ofdb-wasm` exposes:
- `SqlDatabase.open`, `client()`, `execute`, `transaction`, `commit`, `rollback`
- `with_deadline`, `clone_client`

**Must add:**
- `ChangeSubscription` for local change observation
- `synchronize` function accepting `SyncTransport` trait
- `SyncTransport` JS implementation for WebSocket frames
- Owner protocol message types (`SyncHello`, `SyncManifest`, etc.) as TS types

### 13.2 Binding Design

```typescript
// New exports in @ofdb/sql
export interface SyncTransport {
  receive(): Promise<Uint8Array>;
  send(frame: Uint8Array): Promise<void>;
  close(): Promise<void>;
}

export interface SyncConfig {
  direction: 'push' | 'pull' | 'bidirectional';
  maxFrameBytes: number;
  maxSessionBytes: bigint;
  selection: SyncSelection;  // Which tables/rows to sync
}

export async function synchronize(
  client: SqlClient,
  transport: SyncTransport,
  config: SyncConfig,
): Promise<SyncResult>;

// Change subscription for continuous sync
export class ChangeSubscription {
  select(): ChangeWork | null;
  acknowledge(work: ChangeWork): boolean;
  merge(other: ChangeSubscription): ChangeSubscription;
  changed(): Promise<void>;
  run(synchronize: () => Promise<void>): Promise<void>;
}
```

### 13.3 Transport Implementation

```typescript
// WebSocket transport for browser
export class WebSocketSyncTransport implements SyncTransport {
  constructor(private ws: WebSocket) {}
  
  async receive(): Promise<Uint8Array> { ... }
  async send(frame: Uint8Array): Promise<void> { ... }
  async close(): Promise<void> { ... }
}
```

## 14. Glossary References

| Term | Reference |
|------|-----------|
| StoreIdentity | GLOSSARY.md#store-identity |
| ReplicaId | GLOSSARY.md#replica-identity |
| SyncUnitId | GLOSSARY.md#sync-unit-id |
| Tombstone | GLOSSARY.md#tombstone |
| AuthToken | GLOSSARY.md#auth-token |
| ProtocolId | GLOSSARY.md#protocol-id |
| CreationEpoch | GLOSSARY.md#creation-epoch |

## 15. Adoption Status

**This is a proposed protocol draft, not a current implementation contract.**

- Published as stable named topic doc: `docs/sync-protocol.md`
- Linked from `GLOSSARY-MAP.md` when adopted
- Service conversion plan produced separately after proof
- Do not label as implemented until two-store harness passes

---

*End of Protocol Draft*
