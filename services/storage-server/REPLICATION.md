# Native Storage replication lifecycle

This document describes source-checked native replication behavior. It is not a full runtime qualification claim. Target ownership and current migration limits are in [Storage design](../storage-service/docs/design.md).

## Event-driven work

Storage has no periodic full-database reconciliation scan. Policy changes, catalog/open events, and peer-inventory changes rebuild admission.

Each admitted peer/database has one worker for SQL and KV owner exchanges. The sync owner handles initial reconciliation, committed-change coalescing, writes arriving during a pass, and retries. Transport failure retries within that worker without reloading the whole selected-resource inventory.

A per-peer/Resource session gate serializes incoming and outgoing database sessions. Simultaneous initiation yields to the lower endpoint ID. Incoming work queues rather than being rejected merely because outbound work is active.

Each filesystem Resource has a persistent metadata-sync task. Filesystem content and Residency reads use separate authorized paths. Tombstone delivery is a separate one-shot job. Metadata completion alone does not prove that all content is present locally.

## Policy fencing and execution checks

A policy invalidation fences accepted streams and outbound work before a new admission plan is published. Stale plans cannot reopen admission. Removed-peer and canceled jobs are joined; feed closure stops the supervisor.

Handshake admission, endpoint allowlisting, and queued bytes do not authorize later mutation. Transport, frame, send, receive, and apply paths recheck current trusted policy at their execution boundaries. An admitted operation may finish; learned denial blocks later operations.

Selection is desired replication state. Device revocation stops later authorized work when learned, but does not transfer Resource ownership or remotely erase copies. Legacy owner/transfer cleanup in current code is migration work, not the target Device model.

## Host requirements and limits

Install trusted policy-change notifications before starting background replication, and propagate startup errors. A remote HTTP policy client without durable invalidation notifications is insufficient. Do not replace this requirement with a dummy subscription or polling timer.

The host must also maintain the approved endpoint inventory. Inventory changes trigger admission rebuilds, initial catch-up for new peers, and cancellation for removed peers. Local catalog creation/deletion and selected-copy events trigger the required local work, including cleanup with no available peers.

The standalone HTTP-only Storage runtime does not provide native DATA background replication. Its request-time authorization is a separate path. One composed process does not require one physical database file.
