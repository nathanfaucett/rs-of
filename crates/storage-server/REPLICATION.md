# Storage replication lifecycle

Storage has no periodic database reconciliation scan. Admission is rebuilt on trusted
policy changes, local catalog/open events, or peer-store inventory changes.
Each admitted peer/database has one SQL+KV `ChangeSubscription::run` worker. The sync
crate owns initial reconciliation, committed-change coalescing, changes arriving during
a pass, and failed-pass retries. Failed transports retry within that worker; they do
not reload the selected-resource inventory. Both endpoint IDs may initiate database
sync. A per-peer/resource session gate serializes incoming/outgoing database sessions;
simultaneous initiations yield to the lower ID without preventing higher-ID writes.
Incoming sessions queue behind active work rather than being retry-rejected, so an
unavailable lower-ID outbound path cannot starve a working higher-ID inbound path.
A preferred inbound waiter also observes outbound token publication, including when
the outbound serial lock was acquired before its cancellation token was registered.

Each filesystem resource has its own persistent `MetadataSync::run` task. Filesystem
initiation keeps the lower-ID tie-break. Ending a session reconnects with bounded
backoff. These sessions replicate metadata; existing content/residency APIs are
unchanged. Tombstone delivery is a separate one-shot job retained as completed until
its admission changes. Accepted Storage streams and outbound workers share a fenced
admission generation. Policy invalidation immediately cancels both directions and
prevents new sessions, independently of configuration progress. Configuration builds
an admission plan separately from publishing workers. An in-flight cleanup pass may
finish after its own policy writes, but its stale plan cannot reopen admission; the
pending changes coalesce into another pass. Policy notifications are acknowledged
only after canceling and clearing old jobs, including when they arrive after a
catalog wake. Finished desired workers are restarted unless they successfully
completed tombstone delivery. Feed closure stops the supervisor even
when cleanup is blocked. Read-only authority commits are silent, so policy reads do
not feed a reconfiguration loop. Every existing transport/frame/apply authorization
callback checks current trusted policy and peer inventory before and after awaiting
authority evaluation.

## Host integration

Before starting background tasks, install subscriptions from the actual authority
stores, not raw Tokio watch receivers:

```rust
storage.set_policy_changes(
    idp_database.sync_changes()?.merge(management_database.sync_changes()?),
);
storage.start_background_tasks()?;
```

The exact API is:

```rust
StorageRuntime::set_policy_changes(&mut self, changes: ofdb_sql::ChangeSubscription);
StorageRuntime::start_background_tasks(&mut self) -> std::io::Result<()>;
```

`StorageReplicationRuntime` exposes the same setter and
`start_sync(&mut self, CancellationToken) -> std::io::Result<()>` for direct hosts.
Missing policy subscriptions are an explicit startup error when replication is
configured. Hosts must propagate the startup result. Install the feed before reading
initial policy/starting tasks. Closing all merged authority backends fences accepted
sessions as well as outbound replication, rather than retaining old admissions.
The policy watcher and supervisor are bound to the runtime cancellation token;
shutdown joins them, and dropping the runtime cancels its lifetime and aborts both
owned tasks. Cloned DATA handlers cannot keep a dropped runtime's admission alive.

Storage subscribes to `host.server.peers().subscribe_changes()` before its first
configuration pass. Actual inventory changes through `EndpointIdStore::add`, `remove`,
or `replace` (including through a shared clone) automatically reconfigure admission.
Unchanged inventories are silent. No host notification API or post-refresh hook is
needed: a peer refresh after a policy commit is observed independently. New peers get
an initial catch-up; removed-peer workers are canceled and joined even when a
configuration pass is pending. Accepted Storage streams independently observe the
same peer inventory, so removal on the responder alone terminates persistent
filesystem sessions. Other peers retain their workers. Catalog create/delete,
learned tombstones, selected-resource registration and first resource opens notify
locally; ordinary opens of already-cached stores do not cause reconfiguration.

Configuration failures retry with bounded backoff; successful/idle configuration has
no timer. This retry is for failed policy evaluation/local cleanup, not a database
sync scheduler. Local-copy cleanup is evaluated even with an empty peer inventory.

## Standalone remote Management limitation

`ManagementClient` currently provides authenticated request/response policy reads,
not SSE or another remote change subscription. `set_policy_changes` therefore cannot
be wired to a remote Management database by Storage itself. An embedded transport
host must bridge authenticated, durable IdP/Management invalidations into a local
sync-owned subscription and refresh the approved `EndpointIdStore` inventory.
That bridge must cover selection, owner/revocation/cleanup and peer changes, including
catch-up after reconnect. Do not substitute a dummy database subscription or a timer.
Without a bridge, transport replication startup fails explicitly. The current
standalone HTTP-only Storage runtime has no DATA transport/background replication;
its request-time remote authorization is unchanged.

## Default-stack regression coverage

Lifecycle regressions run with explicitly configured 2 MiB Tokio worker stacks and
without `RUST_MIN_STACK` overrides. The SQL catch-up/idle regression uses those same
worker stacks and now passes with the upstream SQL sync stack fix. The replication
suite also covers reconnect after transport rejection with both lifecycles admitted:
only the higher-ID worker's destination is restored, with no new write, policy
commit, or inventory notification to trigger its retry. Transport rejection is
observed explicitly rather than assumed from a fixed startup delay.
