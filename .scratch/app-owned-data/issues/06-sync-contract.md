# 06 — Prove optional sync and prepare the service plan

Status: needs-triage
Work status: open
Type: task
Blocked by: 05

## Goal

Define the protocol an optional backend must implement, using the accepted app as
the consumer. Do not convert production services yet.

## Scope

Draft a backend-neutral protocol covering version negotiation, stable store/replica
identity, explicit attachment, authenticated authorization, browser transport,
framing/limits, cancellation, partial progress, reconnect, conflicts, Tombstones,
acknowledgements, metadata leakage, and key distribution/recovery.

Carry existing ofdb/offs sync protocols; do not replace them with remote CRUD or
copied snapshots. Expose missing SQL owner sync functionality through its JS/WASM
binding. Reuse generic transport machinery where it fits; do not make a browser
run IdP, Management, Device, and Storage to become a sync client.

Use two independent persistent app stores and one small test backend over one
real browser-capable transport. The harness must implement actual protocol
exchange. It is not a production service. Keep KV/filesystem compatibility claims
separate until their own bindings/protocol tests pass.

## Acceptance

- Both stores remain locally usable while disconnected and after restart.
- Offline edits, interrupted transfer, reconnect, and repeated exchange converge
  according to documented owner rules without losing acknowledged local writes.
- Concurrent encrypted credential edits remain recoverable, not silently merged
  or discarded. Deletions do not resurrect from stale peers.
- Wrong store identity, denied access, malformed/oversized frames, incompatible
  version/schema, and learned revocation stop exchange without resetting local data.
- Backend/account changes require explicit attachment and do not replace vault IDs.
- Backend sees no plaintext secrets or unlock keys. State exposed metadata and
  replay/rollback limits honestly. Account access does not imply vault decryption.
- Local saved and sync acknowledged states remain distinct through failures.
- Publish the tested contract in a stable named topic doc and link the glossary
  map when adopted. Do not label the proposal a current implementation contract.

## Later service-plan handoff

Only after the proof, produce a separate plan mapping the accepted protocol onto
existing of services. Identify reusable adapters, new browser transport work,
identity/device authorization changes, retired remote-CRUD assumptions, service
tests, and named design/glossary updates. Do not pre-design their replacement
repositories or create service conversion tickets before this evidence exists.

## Comments

Current Iroh/Storage adapters are useful evidence, not proof of browser protocol
support. E2EE plus CRDT convergence does not imply conflict-free password edits,
all-device durability, immediate global revocation, or secure remote erasure.
