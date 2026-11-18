# Storage API behavior

Exact routes, payloads, status codes, and error schemas belong to OpenAPI: [Storage](../../../openapi/storage.json), [Device](../../../openapi/device.json), [Management](../../../openapi/management.json), and [IdP](../../../openapi/idp.json). This document retains behavior that callers must not infer from schemas alone.

## Namespaces and catalogs

The Storage namespace is `(verified User subject, verified Application)`. OAuth Clients of the same Application share that User's Resources. Other subjects and Applications receive no implicit discovery or access. Storage tokens authorize their exact audience and `read`/`write` actions; they do not authorize mesh replication.

Database and filesystem identities are stable and kind-specific. Display names are optional and non-unique. Unknown, deleted, wrong-kind, and wrong-namespace Resources do not grant access.

Resource deletion creates a Tombstone and removes local runtime/catalog availability. It does not guarantee immediate removal of every physical or offline copy. Deselection is a separate desired-replication operation. Full/Passthrough Residency is local filesystem policy, not Selection, Resource ownership, or proof of content availability.

## Explicit database creation

Database ensure is explicit creation or reopen; authentication and metadata reads do not provision a database. A stable creation key identifies a logical Resource within the verified subject/Application namespace. Retrying the same key converges on the same derived Resource identity; changing it deliberately selects a different identity.

The current identity derivation uses UUIDv5 over the verified subject, canonical Application ID, and creation key. Names remain non-unique labels. Disconnected Nodes can derive the same identity without global create serialization or globally ordered writes.

Initialization publishes valid existing database content before exposure. Interrupted fresh initialization can resume from durable intent. Missing or corrupt acknowledged content is an error, never a reason to create an empty replacement. A learned Tombstone permanently consumes that identity; recreation requires a new key. Local-copy fences remain distinct from unknown Resource lookups.

## SQL batch transactions

One SQL HTTP batch executes its statements in order in one server-owned transaction. The server validates the supported statement shapes and required permissions before execution. It commits only after all statements, result validation, and limits succeed; otherwise it rolls back. There are no client transaction handles or automatic mutation replay.

SELECT requires `read`; DDL and mutations require `write`; INSERT/UPDATE/DELETE with RETURNING require both. Mixed batches need every required action. The endpoint is a bounded CRUD contract, not unrestricted SQL hosting. Joins, subqueries, aggregates, computed projections, and unsupported DDL are rejected rather than silently approximated.

Current wire bounds include a 256 KiB request body, 1–32 statements, 16 KiB SQL per statement, 128 parameters per statement, 1,000 returned rows, and 1 MiB serialized results. Native read/scan budgets also apply, including for small or empty projected results. Limit failure aborts the whole batch rather than truncating a snapshot.

Authorization and namespace are rechecked around transaction admission and response publication without holding an owner transaction across identity or network calls. Denial after commit can suppress the response but cannot undo committed work. Client timeout or cancellation after send is not proof of rollback; reconcile before retrying.

## Device target and current legacy behavior

The target Device context owns Device records and configuration. Management only authorizes administration. Selection maps a Resource to an Installation-level Device and can serve multiple permitted User/Application namespaces on that Device.

Current contracts and implementation still include single-User Device owner/transfer behavior and legacy cleanup. These are not the target model and do not prove migration is complete. Device revocation never transfers Resource ownership or remotely erases copies. See [Storage design](design.md) and [ADR 0002](../../../docs/adr/0002-shared-installation-devices.md).
