# Remote control-plane notes — tickets 03 / 04

Read-only inspection of the main `of` / `ofdb` source trees; no worktrees, target, or node_modules scanned. Only this report written. Findings are source-verified; no builds or tests run.

## Smallest viable query adapter

**Reuse `ofdb_sql::Client`; do not introduce another local/remote enum, repository family, or query trait.** It already wraps embedded and remote backends (`ofdb/crates/sql/src/client.rs:38`). Local construction is `database.client()`; remote construction is `Client::connect`, `connect_with_ca`, `connect_with_identity`, or `connect_unix`. The cloneable client exposes typed `execute(Vec<Statement>) -> Vec<QueryResult>`, SQL translation, row decoding, transactions, and optional deadlines. SQL is translated client-side into typed statements; this is not a remote raw-SQL parser.

- `of/crates/db/src/lib.rs` already re-exports this facade `Client` and `Transaction`, **not** the lower-level `ofdb-sql-client::Client`. `of/crates/db/Cargo.toml` currently enables SQL/redb/in-memory/macros, but not `ofdb-sql/remote` or `server`; add targeted feature forwarding. Current `replica` also implies native, so a remote-only build needs feature decoupling if required, not just another flag.
- Inject a cloned `db::Client` into existing concrete repositories and query-only router state; replace `Arc<Database>` fields and `.client()` calls. Keep existing repository interfaces and service code. Preserve the owner `Arc<Database>` separately for local hosting, sync, lifecycle, and migration ownership. Cloning a local client shares the database; it does not reopen the file or use gRPC.
- Main consumers: `of/crates/idp-service/src/replica/*.rs` (repositories actually live here, not in idp-model); `of/crates/management-service/src/replica/{role_repo,permission_repo,device_repo,selection_policy}.rs`; `of/crates/management-service/src/local_permissions.rs`.
- `ofdb/crates/sql-protocol/src/executor.rs` is a server-side executor interface, not the missing consumer adapter. `sql-server/src/executor.rs` already implements it for the engine. `sql/src/database.rs:442` already provides owner-side TCP/TLS/mTLS hosting.

## Blocking gap: conflict semantics are not remotely implemented

**Changing constructors alone will compile toward the seam but will not make tickets 03/04 operational.** The facade deliberately returns `QueryErrorKind::Unsupported` for remote `row_conflicts`, `row_conflicts_batch`, `row_conflict_values`, `resolve_row`, and transaction conflict inspections. See `ofdb/crates/sql/src/client.rs:84–138,290–406`; existing assertions in `ofdb/crates/sql/tests/remote.rs` explicitly expect this behavior. Wire interface currently has only Execute and streaming Transaction (`ofdb/crates/sql-proto/proto/db.proto:484`).

| Actual caller | Required behavior missing remotely |
| --- | --- |
| `of/crates/idp-model/src/replica.rs:118–242` | Authentication, authorization, token issuance, signer trust use conflict/batched-conflict inspection; administrator resolution uses inspection + resolve. These helpers also require `&Database` today. |
| `of/crates/idp-service/src/replica/{user_repo,client_repo}.rs` | Secret-row conflict checks; transactional credential/client lifecycle checks. |
| `of/crates/idp-service/src/replica/{key_repo,oauth2_authorization_code_repo,oauth2_refresh_token_repo,replica_signer_repo}.rs` | Indirect conflict clearance through idp-model helpers. |
| `of/crates/management-service/src/replica/{role_repo,permission_repo}.rs` | `ensure_clear` calls `allows_authorization`, so even roles/permissions are not plain CRUD. |
| `of/crates/management-service/src/replica/{device_repo,selection_policy}.rs` | Direct and transaction-scoped conflict checks; selection convergence reads conflicting values and resolves rows. |

Smallest safe completion: extend the existing ofdb wire/client/executor/facade with the conflict operations these callers use, including checks inside the **same remote transaction snapshot**. Update `sql-protocol/src/executor.rs`, `sql-server/src/{executor,service}.rs`, `sql-client/src/{client,transaction}.rs`, and `sql/src/client.rs` together. Do not replace Unsupported with empty conflicts or move checks outside transactions. Removing conflict logic would be a separate security/domain redesign, not adapter parity.

## Startup and ownership seams

| Location | Current behavior / required change |
| --- | --- |
| `of/crates/idp-server/src/cli/run.rs:145–153` | Opens `idp.redb`, migrates, calls `build_oauth_runtime`. Select remote connection before this local-open path. |
| `of/crates/idp-server/src/runtime.rs:131–245`, `router/state.rs:33` | Existing injection seam (`build_runtime`, `build_oauth_runtime`, `build_state`), but all accept/store `Arc<Database>`. Convert query dependencies to client; keep owner capabilities outside request state where needed. |
| `of/crates/management-server/src/cli/run.rs:163`, `provisioning.rs:15` | `open_database` opens/migrates `management.redb`; remote consumers must bypass it. |
| `of/crates/management-server/src/runtime.rs:70–99` | `build_router` already accepts an externally initialized handle, but builds roles **and Devices/policies** together. Ticket 01/02 split is prerequisite to clean identity/Device injection. |
| `of/crates/unified-server/src/runtime.rs:149–177` | `RuntimeOwners::open` currently opens separate IdP and Management files. Replace with identity + Device owners, migrate each once, derive clients, inject identity into IdP/Management and Device into Device authority/local admission. No SQL listener needed here. |
| `of/crates/idp-server/src/provisioning.rs:70–101`; both servers' `independent_create.rs`, `independent_join.rs` | Provisioning has its own local-open and owner-handle assumptions. Updating normal serving alone leaves setup/create/join opening private databases. Route these through the same explicit startup selection/injection. |

Both `config/app_config.rs` files lack SQL endpoint/CA/client identity configuration. Add validated startup configuration: endpoint, CA, optional client cert+key, finite query deadline; reject conflicting owner/injected/remote modes and fail startup on remote errors rather than falling back to local files. A remote URI does not replace the local runtime/setup directory.

The generic `ofdb/crates/sql-server-cli/src/main.rs:43–64` opens and serves a database but runs **no application migrations**. Minimal domain owner is a small `of` startup path that opens one domain database, applies its schema, then calls existing `serve_tcp_with_mtls`. Run distinct identity and Device owners/listeners; do not teach generic ofdb about IdP tables.

## Authentication, TLS, transactions, migrations, reset

- **SQL authentication:** TCP is unauthenticated by default. Server supports certificate/key and client-CA verification; facade supports matching mTLS credentials. Neither inspected SQL handler nor client attaches/verifies OAuth bearer metadata or implements per-table authorization. A trusted certificate gets the exposed query interface, not role-limited SQL access. Use separate domain credentials/trust configuration and private listeners; require TLS + client authentication for nonlocal deployments. Current CLI defaults to `0.0.0.0:8081` with TLS optional. A connection string alone is not a secure deployment contract.
- **Transactions:** Already available locally/remotely through `db::Transaction`; lower-level implementation is `ofdb/crates/sql-client/src/transaction.rs`, server lifecycle is `sql-server/src/service.rs:59–157`. Stream disconnect/drop rolls back uncommitted work; execution failure aborts the transaction and prevents commit. Keep credential/token/Device multi-step operations on one handle. Facade `with_deadline` uses Tokio time; timeout is explicitly **not proof of rollback**. Do not blindly retry ambiguous writes/commits. Missing capability is conflict inspection, not BEGIN/COMMIT transport.
- **Migrations:** `of/crates/idp-model/src/replica.rs:7` executes embedded SQL files statement-by-statement, then rejects mixed credential history. `of/crates/management-service/src/replica/schema.rs:3–42` similarly creates combined authorization + Device tables and checks legacy enrollment layout. No migration version ledger/whole-run transaction is present in these entry points. Consolidate authorization DDL under identity ownership and Device DDL under Device ownership; consumers should validate readiness, not run private copies. Identity SQL currently also contains Device selection tables (`of/crates/idp-model/replica_migrations/20241117224814_init.up.sql:223`), so the split must remove those too. Do not silently migrate unsupported historical layouts.
- **Signing secrets:** Shared SQL is not sufficient signer provisioning. Runtime constructs `PrivateKeyKeyringRepo`, whose private material remains in the OS credential store (`of/crates/idp-service/src/repo/private_key_keyring_repo.rs:42–74`); SQL key rows carry public metadata. Preserve explicit IdP-local signer provisioning; Management/SQL hosting must not become token-signing owners.
- **Reset:** `of/crates/idp-server/src/independent_reset.rs:136–151` assumes local `idp.redb` and constructs a provisioner for secret cleanup; Management reset also retains local owner inventory. `of/crates/api/src/independent_reset.rs:247–286` ultimately removes the local domain/retired directory. Keep shared database paths outside consumer reset ownership; remote reset drops client/runtime state and only that runtime's local secrets. Do not reuse database retirement/removal for a consumer reset. Add a test proving other consumers retain rows.

## Ticket 04: SQL transport is not Storage admission

Ticket 04's problem/test wording conflates querying SQL with asking the Device authority. Follow the spec and ticket's implementation decision: **Device server → SQL gRPC database; Storage → authenticated Device admission interface**. No Device-server crate exists under the inspected `of/crates` yet (ticket 02 prerequisite); its external transport still needs to be made explicit, not inferred from SQL gRPC.

Reuse/move the existing authenticated remote-policy path:
- `of/crates/storage-server/src/management_client.rs`: OAuth client-credentials, issuer/scope checks, HTTPS client, bounded requests, admission/selection/cleanup calls; rename/retarget toward Device authority with new scopes/audience/contracts.
- `of/crates/storage-server/src/local_replication_authorizer.rs:33–165`, `runtime.rs:76–141`: existing local callback / remote client injection seam; preserve endpoint-owner binding and transfer cleanup/revalidation.
- `of/crates/unified-server/src/runtime.rs:1–147`: current local authorizer captures Management database/policy repositories; redirect to the Device owner.

Keep unknown/revoked/conflicted/unreachable authority decisions fail-closed. Do not give Storage raw SQL credentials or alter user-resource iroh replication.

## Implementation order / acceptance checks

1. Land ticket 01/02 ownership split; use the existing facade client as the query seam, retaining database owners separately.
2. Implement/test remote conflict parity in ofdb, then convert idp-model clearance helpers and concrete repositories to that client.
3. Add domain SQL-owner startup and validated remote consumer configuration across serving **and setup/join/reset** paths.
4. Prove external behavior: independent IdP + Management processes share authorization changes; Unified sharing works with no gRPC listener; Device-backed admission permits approved endpoints and rejects unknown/revoked/unavailable authority; consumer reset preserves shared rows.
5. Extend `ofdb/crates/sql/tests/remote.rs` for conflict/transaction parity and mTLS rejection, then server-level coverage near `of/crates/idp-server/src/permission_evaluation_tests.rs`, `of/crates/unified-server/src/permission_http_tests.rs`, and `of/crates/storage-server/src/local_replication_authorizer_tests.rs`. Existing remote transaction/mTLS tests are prior art, not evidence the application path already works.
