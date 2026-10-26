# 02: Use SDK-bound Storage in the password manager

**What to build:** A signed-in User discovers, creates, and opens filesystem Storage Resources through SDK-bound Storage access, then uses the existing collection-backed password manager for CRUD and explicit Refresh. Ending or replacing the authenticated context clears its data and connections safely. Follow the parent Application SDK specification.

**Blocked by:** 01 — Sign in and out through @of/sdk.

**Triage:** ready-for-agent

**Status:** resolved

- [x] Supply context-bound filesystem discovery, creation, and opening through the existing Storage client, using the existing OIDC client's token renewal and Storage client's token exchange. Preserve explicit service bases, stable Filesystem IDs, and server-side User/Application authorization; discovery does not grant access.
- [x] Compose this capability with the completed Storage collection adapter at its existing Storage seam. The SDK and adapter do not depend on each other; neither imports Svelte or app schemas. Keep resource selection, Secret rules, editor state, and official TanStack Svelte live queries in the app.
- [x] Retired handles cannot initiate new requests, acquire a new context's token, or surface obsolete discovery/open results as current success. Close connections that finish opening after retirement. Already-sent writes are not promised to be canceled or automatically replayed.
- [x] Define one connection handoff and cleanup owner: the SDK cleans failed/obsolete/untransferred opens; the adapter cleans adopted connections through its existing disposal operation. Any necessary context guard has idempotent close semantics and does not become a collection registry or second reconciliation engine.
- [x] Context retirement triggers application disposal of old collections and clears editor selection; filesystem-only switching disposes the selected collection without ending authentication. Late read/write/refresh completions cannot populate or edit the new context. Preserve normal same-identity renewal without unnecessary collection teardown.
- [x] Preserve CRUD, validation, safe paths, history and unedited fields, optimistic failure behavior, and explicit Refresh from the prerequisite integration. Remove superseded application Storage/token wiring rather than maintain a bypass around the SDK.
- [x] Test the public SDK-to-collection workflow with real clients and the real TanStack runtime against controlled responses, including distinct service prefixes, denied access, late discovery/open, obsolete handles, connection ownership, pending mutations during context termination, and repeated disposal.
- [x] Run a focused full-app smoke workflow covering sign-in/callback, filesystem discovery/create/open, CRUD, documented reload behavior, explicit Refresh, filesystem switching, and sign-out. Verify no old records or editor state remain after a context change.
- [x] Document SDK/client/adapter/application responsibilities, credential and logout semantics, setup, refresh limits, and a real-system qualification path. Do not imply mocked transport proves Node replication, remote revocation, push synchronization, or production vault encryption.
- [x] Run package tests, TypeScript/Svelte checks, applicable lint/format checks, production build, and available complexity validation. Report unavailable checks and pre-existing failures; run required Rust validation only if Rust changes occur.
- [x] No custom Svelte wrapper, Management/Device setup dependency, generic provider framework, polling, automatic reconnect, browser-offline outbox, new collection implementation, or separate hardening phase is introduced.


## Answer

Implemented on `implementation/application-sdk-02` in the isolated `application-sdk-02` worktree. Confirmed before edits: HEAD and `integration/application-sdk` were both `e5d3c305a401f34aba4d7f1e8891d9aab4ed090d`; `git merge-base --is-ancestor e5d3c305a HEAD` succeeded, including resolved ticket 01.

- Added frozen `ApplicationSdk.storage()` capabilities for list/create/open through the real Storage client with immutable explicit prefixes/client configuration and a private generation-captured OIDC token provider. Removed the public transitional token-provider API. OIDC still owns renewal/verified identity; Storage owns exchange and resource protocol. No token cache, collection registry, protocol replacement, or SDK/adapter dependency.
- Added the minimal context connection guard. Requests reject before enqueueing, before queued sends, and after late results; new requests reject immediately even behind outstanding work. Its event-only socket surface does not expose raw `send` or credential-bearing socket URLs. Close is idempotent. Context retirement aborts untransferred attempts; failed/canceled/late opens are cleaned before rejection. Successful `openSocket()` resolution transfers cleanup to the recipient, and the adapter owns adopted connections through existing cleanup. Already-sent writes may persist; nothing retries mutations automatically.
- Added a structural `StorageConnection` type at the Storage owner seam and narrowed the adapter's accepted client to its existing open/request/close/event requirements. Adapter record validation, safe paths, optimistic settlement, reconciliation, Refresh, deadlines, and disposal implementation are unchanged. Related generated type declarations are retained; unrelated generated runtime/cache/install changes are restored.
- Migrated the app directly to SDK-bound Storage and deleted `storageSocket.ts` plus its fake-token tests. Filesystem selection, Secret/history/editor rules, query bindings and synchronous authenticated-view teardown stay in the app. Filesystem-only switching retains authentication. Same-identity renewal demonstrably retains the exact session, socket, collection and editor.
- Expanded real-client SDK workflows and removed the mounted collection tests' fake SDK/token-provider fallback. The full mounted app smoke uses real SDK/OIDC/Storage/adapter/TanStack runtime with controlled HTTP/navigation/WebSocket responses for sign-in, the actual callback route in a fresh SDK runtime, authenticated-layout restoration, discovery/create/open, CRUD, fresh-runtime reload restoration, demand-driven renewal, explicit Refresh, filesystem switching, and sign-out. Tests also cover stale list/create/open/token exchange, callback generations, denied versus transient failures, retained/queued requests, cancellation, idempotent handoff cleanup, and pending writes during User change/sign-out.
- Updated SDK/app/adapter READMEs with composition and one-owner handoff, persistence/local logout semantics, freshness/cancellation limits, setup and an expanded real-system qualification procedure.

Validation passed:

- `pnpm --filter @of/sdk test` — 40 tests; real OIDC suite — 14; Storage client — 21; collection adapter — 59; password manager — 20 including mounted full workflow. App config Node tests — 2.
- OIDC TypeScript no-emit check; Storage client, SDK and adapter TypeScript builds; adapter no-emit check; password-manager production build; Svelte check (0 errors/warnings).
- Focused changed-source Biome lint/format; Svelte autofixer on both touched components (no issues; Browser retains advisory suggestions about its existing asynchronous loading effect); `git diff --check`.
- `just crap` attempted in this worktree and failed before coverage at Cargo metadata: nested layout cannot find sibling `.scratch/worktrees/ofdb/crates/sql/Cargo.toml`. It is Rust-only; no Rust source changed and Rust feature-powerset tests were not applicable. Complexity coverage can be rerun from the main checkout with the real sibling workspaces after integration.

Qualification limits: controlled services/mounted jsdom are not a deployed browser/provider/Installation test. No live Installation or credentials were available; provider logout, remote revocation, Node replication/convergence, cross-client push freshness and production vault security remain unqualified. Plaintext browser credential persistence/XSS exposure and no browser secret persistence/encryption remain documented. No push performed.
