# Password manager: application-provisioned SQL

Sign in through `@of/sdk` and the app automatically opens its User/Application database. The app-owned SQL collection retains the **official** `@tanstack/svelte-db` binding and collection mutation path. No resource selector, database ID, or manual creation step is required.

**Cutover:** existing filesystem records remain untouched. They are not imported, read as fallback, dual-written, or deleted. A fresh SQL database is separate from those records. This example adds **no end-to-end encryption, secure erasure, production-vault qualification, or concurrent-edit protection**. Use non-sensitive test secrets only.

## Local setup

From the workspace root:

```sh
pnpm install --frozen-lockfile
pnpm --filter @aicacia/idp-client build:esm
pnpm --filter @aicacia/native-fetch build:esm
pnpm --filter @aicacia/oidc-client build:esm
pnpm --filter @aicacia/storage-client build:esm
pnpm --filter @of/sdk build
pnpm --filter @aicacia/svelte-headless prepack
```

Copy `.env.example` to `.env` and configure `PUBLIC_URL`, `PUBLIC_IDP_BASE_URL`, and `PUBLIC_STORAGE_BASE_URL` for a provisioned Installation. Keep service prefixes (`/idp`, `/storage`); Storage need not share the IdP host. `PUBLIC_URL` must match the actual browser origin/base and registered callback URI. Use browser-trusted HTTPS and allowed browser origins; never disable certificate validation or authorization. Public configuration contains locations, not credentials.

```sh
pnpm --filter password-manager dev
```

Use an Installation User allowed to onboard Applications. The app supplies Application URI `password-manager.aicacia.com`, not an invented OAuth Client ID. IdP authorization registers/consents and returns its issued Client ID with code, PKCE-bound state, and issuer. OIDC verifies the callback, userinfo, and authenticated Client/Application mapping. Reload and renewal retain that issued ID. See [SDK authentication and credential policy](../../packages/sdk/README.md).

The sign-in form can select an IdP service base, retained in the existing `idp-api-url` preference. Storage is independently configured. Configuration changes dispose the old SDK. `PUBLIC_LIDP_BASE_URL` is obsolete.

## Ownership, identity and schema

- `src/lib/storage/startup.ts` alone chooses `password-manager:default` as the permanent logical creation key and explicitly calls `storage.ensureDatabase({ creationKey, name })`. Display name is only a label. No list/name lookup, browser-stored database preference, replacement key, or unkeyed creation fallback exists.
- Storage derives identity from verified User/canonical Application/key, so same-Application Clients, retries, and concurrent tabs address the same intended resource on a Node. Disconnected Nodes derive the same identity, **not** globally serialized writes. See the [generic identity/lifecycle contract](../../crates/storage-service/docs/api.md#explicit-application-sql-contract).
- A Tombstone permanently consumes the key. Deleted (`410`), fenced (`409`), absent, unavailable (`503`), corrupt, or denied resources fail closed. **Retry preparation** repeats the same explicit operation; it cannot recreate a learned deleted resource or replace acknowledged unavailable content with an empty database. Fix authorization/connectivity/content through operator recovery, not a new application key.
- An empty owner catalog is initialized with `secret_schema` and `secrets` plus schema version `1` in one atomic SQL batch. Repeatable DDL and insert-on-conflict-do-nothing support concurrent initialization and lost-response retry without overwriting a version. Existing catalogs must contain both expected tables, and the version query must return exactly the known marker/version. Missing/duplicate/malformed or incompatible version state fails without automatic repair, reset, or migration.
- `secrets` stores UUID ID and validated serialized JSON text. New records use UUIDv7, accepted by the SQL owner's row-identity contract. Nested tables, speculative indexes, and SQL ordering are unnecessary; the TanStack query orders favorite-first, then name.
- The SDK/client only provide explicit generic database operations and session guards. Neither SDK nor collection provisions defaults. The collection takes an already resolved Database ID and structural `executeSql` capability, not a concrete SDK. Device enrollment, selection, and replication permissions remain separate.

## Editor, reconciliation and recovery

Preparing, loading, ready-empty, ready, and failure states are distinct. No partial or malformed snapshot is published as an empty/valid vault. Loaded JSON, schema, row identity, result shape, and duplicate IDs are validated before one complete collection publication. Reads are one bounded SQL SELECT rather than sequential file reads.

New secret, Save, and Delete use TanStack mutations, with optimistic list changes. Required Name/Secret validation occurs before writing. Edits retain additional websites and match modes, custom fields, unedited notes, timestamps, and history. Clearing Website removes only the first website. Changing Secret appends its previous value and timestamp; other edits do not add history. Save advances `updatedAt` while retaining `createdAt`/`deletedAt`.

Each mutation and its authoritative full `SELECT id, content FROM secrets` snapshot execute in the **same atomic SQL batch**, with both read and write actions. A snapshot crossing 1,000 rows or 1 MiB rejects and rolls back the mutation, leaving the last readable vault intact rather than committing an unrepairable oversized vault. The returned snapshot is fully validated before publication; acknowledged writes do not require a second post-commit read. Rejected/uncertain responses still trigger a separate authoritative read without mutation replay. Other native read/decoded-work budgets can reject unusually large or history-heavy resources before wire-result validation; see the [HTTP SQL contract](../../crates/storage-service/docs/api.md#explicit-application-sql-contract).

Controls are disabled through persistence and authoritative reconciliation. Success is reported only after settlement; an acknowledged mutation whose returned snapshot does not match the intended outcome is uncertain, not success. This observation is **not** compare-and-swap or prevention of a later concurrent edit.

- Definitive pre-commit SQL rejection reconciles and reports an error while retaining safe drafts.
- Missing/malformed acknowledgements, network failure, timeout, and potentially post-commit authorization failure are **uncertain**. After successful reconciliation the error remains and the draft closes. Inspect the result before deciding on another mutation. Writes are never automatically replayed.
- Failed reconciliation hides records/drafts, disables edits, and requires explicit **Refresh**. Each ensure/SQL operation, including token exchange, has a 30-second deadline. A timed-out request is locally aborted; that does not prove remote rollback. A remote operation may finish even after the first reconciliation; Refresh and verify again when needed.
- **Refresh** reloads external additions/edits/deletions on the same collection and closes stale drafts on success. It cannot overlap active writes in the UI. Failed/malformed refresh hides untrusted records and offers explicit retry, not a falsely empty list. There is no polling, automatic reconnect, browser-offline queue, or cross-tab change feed.
- Sign out, identity change, SDK retirement, and route teardown synchronously clear records/drafts and dispose queries before source collections. Captured cancellation/session guards suppress obsolete results/notices. Same-identity renewal preserves the current collection/editor. Storage denial/outage does not itself end IdP authentication; terminal IdP identity/credential rejection does.

Decrypted records and drafts live only in memory. No secret cache, IndexedDB, localStorage secrets, or durable mutation outbox is added. OIDC separately persists plaintext credentials and pending PKCE transactions in its namespaced localStorage policy for redirect/reload continuity. XSS, extensions, and local inspection can expose those credentials and in-memory secrets. Local sign-out is not provider logout/revocation; credential-removal failure requires site-data cleanup.

Storage receives plaintext serialized secret values. SQL/CRDT history and offline/replicated copies may retain overwritten/deleted values. TLS is transport protection, not client-side encryption or secure deletion. Do not log tokens, SQL parameters, secret records, or unredacted network traces. Generic filesystem APIs/collections remain available to other consumers; this app no longer depends on them.

## Repeatable checks

```sh
pnpm --filter password-manager test
pnpm --filter password-manager exec node --test svelte.config.test.mjs
pnpm --filter password-manager build
pnpm --filter password-manager check
pnpm --filter @of/sdk test
pnpm --filter @aicacia/storage-client test
cargo test -p idp-unified --test owner_boundaries sql_http_tests --no-default-features
```

Build first in a clean checkout to generate the existing Paraglide runtime before type checking. Source lint/format checks should exclude generated `.svelte-kit`, build, and Paraglide output. Biome's Svelte script analysis can flag template-used symbols as unused; `svelte-check` and the Svelte autofixer validate those components.

Mounted app tests use the real SDK, OIDC/Storage clients, SQL collection, and official binding with controlled HTTP. They exercise fresh callback/restoration, provisioning, explicit retry, schema checks, CRUD/metadata/history/order, reload, Refresh, malformed snapshots, rejected/uncertain writes, deadlines, retirement races, and same-identity renewal. Native `sql_http_tests` independently exercise real IdP exchange/authenticated HTTP/catalog/SQL persistence, including the app's schema/CRUD request sequence and Node-local runtime restart. Controlled transport fixtures do not prove server security or durability; native fixtures do not qualify a deployed browser Installation.

## Real-system qualification (manual; not performed for this ticket)

No configured running Installation/browser credentials were supplied. Before deployment record commit, Node versions, redacted service bases, browser/version, User/Application labels, test time, and observations. Use test secrets only.

1. Complete actual browser IdP sign-in/token exchange. Verify automatic preparation and a ready empty database without a selector. Open two tabs simultaneously; confirm the intended resource identity through authorized operator tools, not name matching.
2. Create, edit, favorite, and delete records. Reload the browser after each operation. Seed additional websites/match modes, notes, custom fields, timestamps/history through an authorized test client and verify preservation. Restart the Node, reopen, and verify the same records/version.
3. Add/edit/delete externally, then Refresh. Verify ordering, external changes, closure of stale drafts, and disabled Refresh during writes. This checks snapshot freshness, not concurrent-edit conflict prevention.
4. Deny access or interrupt connectivity during preparation, reads, and writes. Existing content must not turn into a ready empty replacement or trigger sign-out solely for a Storage failure. Restore authorization/connectivity and explicitly retry/Refresh. Withhold responses for 30 seconds; verify uncertain writes are not replayed. Verify outcomes before retrying.
5. Test incompatible/missing schema state, malformed JSON/records, result-size failure, deleted resources, unavailable acknowledged content, and local-copy fences with disposable test data. Expect bounded errors, no partial snapshots, and no automatic repair/replacement. Restore content via operator recovery before explicit retry.
6. Sign out, change User/IdP, and navigate away during preparation/load/Refresh/write. Records/drafts disappear immediately; late outcomes never enter the replacement session. Force same-identity renewal separately and verify its editor stays intact. Test terminal IdP rejection separately from Storage denial. A sent write may still persist after retirement.
7. Qualify replication separately with authorized selected Nodes, partitions/recovery and the platform operations procedures if replication is claimed. Same-Node Refresh/native restart and equal derived identities do not establish mesh/catalog convergence, global single-writer semantics, or end-to-end encryption.
