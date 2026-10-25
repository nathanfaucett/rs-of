# 02: Use SDK-bound Storage in the password manager

**What to build:** A signed-in User discovers, creates, and opens filesystem Storage Resources through SDK-bound Storage access, then uses the existing collection-backed password manager for CRUD and explicit Refresh. Ending or replacing the authenticated context clears its data and connections safely. Follow the parent Application SDK specification.

**Blocked by:** 01 — Sign in and out through @of/sdk.

**Status:** ready-for-agent

- [ ] Supply context-bound filesystem discovery, creation, and opening through the existing Storage client, using the existing OIDC client's token renewal and Storage client's token exchange. Preserve explicit service bases, stable Filesystem IDs, and server-side User/Application authorization; discovery does not grant access.
- [ ] Compose this capability with the completed Storage collection adapter at its existing Storage seam. The SDK and adapter do not depend on each other; neither imports Svelte or app schemas. Keep resource selection, Secret rules, editor state, and official TanStack Svelte live queries in the app.
- [ ] Retired handles cannot initiate new requests, acquire a new context's token, or surface obsolete discovery/open results as current success. Close connections that finish opening after retirement. Already-sent writes are not promised to be canceled or automatically replayed.
- [ ] Define one connection handoff and cleanup owner: the SDK cleans failed/obsolete/untransferred opens; the adapter cleans adopted connections through its existing disposal operation. Any necessary context guard has idempotent close semantics and does not become a collection registry or second reconciliation engine.
- [ ] Context retirement triggers application disposal of old collections and clears editor selection; filesystem-only switching disposes the selected collection without ending authentication. Late read/write/refresh completions cannot populate or edit the new context. Preserve normal same-identity renewal without unnecessary collection teardown.
- [ ] Preserve CRUD, validation, safe paths, history and unedited fields, optimistic failure behavior, and explicit Refresh from the prerequisite integration. Remove superseded application Storage/token wiring rather than maintain a bypass around the SDK.
- [ ] Test the public SDK-to-collection workflow with real clients and the real TanStack runtime against controlled responses, including distinct service prefixes, denied access, late discovery/open, obsolete handles, connection ownership, pending mutations during context termination, and repeated disposal.
- [ ] Run a focused full-app smoke workflow covering sign-in/callback, filesystem discovery/create/open, CRUD, documented reload behavior, explicit Refresh, filesystem switching, and sign-out. Verify no old records or editor state remain after a context change.
- [ ] Document SDK/client/adapter/application responsibilities, credential and logout semantics, setup, refresh limits, and a real-system qualification path. Do not imply mocked transport proves Node replication, remote revocation, push synchronization, or production vault encryption.
- [ ] Run package tests, TypeScript/Svelte checks, applicable lint/format checks, production build, and available complexity validation. Report unavailable checks and pre-existing failures; run required Rust validation only if Rust changes occur.
- [ ] No custom Svelte wrapper, Management/Device setup dependency, generic provider framework, polling, automatic reconnect, browser-offline outbox, new collection implementation, or separate hardening phase is introduced.
