# 01: Sign in and out through @of/sdk

**What to build:** The password manager uses one framework-independent SDK instance for configuration, sign-in, callback handling, session restoration, and safe local sign-out, including when the IdP is unreachable. Follow the parent Application SDK specification.

**Blocked by:** TanStack DB integration tickets 02–04 — Open a filesystem through TanStack DB; Save and delete secrets through the collection; Refresh and qualify the example. Complete that effort before starting this follow-on migration.

**Triage:** ready-for-agent

**Status:** resolved

- [x] Add the framework-independent `@of/sdk` workspace package and migrate the password manager's authentication workflow with it; do not ship standalone scaffolding without a working consumer.
- [x] Accept immutable explicit IdP/Storage service bases, OAuth Client configuration, Application metadata, and redirect configuration. Preserve service prefixes and distinct Application, client, and issuer identities. Keep environment mapping, navigation, safe return-to handling, route guards, and notifications in the application.
- [x] Delegate discovery, registration, authorization-code/PKCE, callback validation, token renewal, user information, credential persistence, and provider logout to the existing OIDC client. Fix correctness gaps in that client rather than duplicate protocol logic in the SDK.
- [x] Isolate cached credentials and pending authorization transactions across OAuth Clients on the same authority. Clearing one context must not purge unrelated credentials; do not preserve unsafe legacy cache compatibility.
- [x] Establish authenticated state through the supported verified identity workflow, not token presence or unverified decoded claims. Invalid/no-op callbacks cannot report authenticated success. Document and test credential persistence, full-page redirect continuity, and reload restoration; memory-only PKCE state is not sufficient for a full-page redirect.
- [x] Expose only a minimal token-free read-only session snapshot and subscribe/unsubscribe mechanism. Same-identity renewal retains the context; ending or replacing the context invalidates old work before network waits. Transient connectivity errors and resource denials must not indiscriminately end authentication.
- [x] Local sign-out clears local credentials and authenticated app state even when provider discovery/logout fails; report provider failure separately without claiming revocation. Disposal is idempotent, stops new work, and prevents late callback/renewal results from reviving a retired context.
- [x] Delete superseded app authentication composition without compatibility facades. Keep the existing collection-backed app usable during this slice; retained Storage wiring must observe the SDK lifecycle so sign-out clears old collections and editor state from the outset.
- [x] Test public SDK workflows using real OIDC behavior with controlled service/platform responses: sign-in/callback, invalid state, restoration, same-authority client isolation, expiry/renewal, sign-out with an unreachable provider, unsubscribe, and disposal during authentication. Do not log credentials, callback parameters, PKCE verifiers, or secrets.
- [x] Run relevant package and app checks and update workflow/security documentation. No custom Svelte package, second token cache, background auth scheduler, or new OAuth flow is introduced.


## Answer

Implemented on `implementation/application-sdk-01` in the isolated `application-sdk-01` worktree, based on confirmed integration ancestor `c73f24cb0` (the completed collection prerequisite is already included).

- Added the framework-independent workspace `@of/sdk` with immutable explicit service/client/Application configuration, token-free frozen session snapshots, subscriptions, callback/restoration, existing sign-in/registration delegation, local-first sign-out, optional separately reported provider logout, captured generation-bound token providers, and idempotent disposal. Each new callback retires previous work before awaiting identity; late discovery, exchange, userinfo, or renewal results cannot revive or retire a replacement context.
- Fixed the owning OIDC client: scoped credential/PKCE namespaces, no legacy authority-cache reads or broad purges, state-bound authorization-code callbacks only, destination validation, trusted discovery/userinfo identity establishment, guarded callback writes, coalesced renewal, local clearing before logout discovery, and removal of authorization-URL/provider-body diagnostics. Storage service-base validation is reused from the Storage owner client. No new OAuth stack or token cache was added.
- Replaced the app's old OIDC/IdP auth modules with one SDK composition. IdP selection is persisted synchronously before redirect and never silently follows another tab's preference. Guards/callbacks use SDK identity; local sign-out/disposal synchronously removes mounted collections, sockets, and editor state through the existing adapter. Same-identity renewal retains the collection. App-level configuration and the full collection-backed workflow are tested with real SDK/OIDC behavior and controlled services/navigation.
- Documented plaintext localStorage credentials/PKCE redirect continuity, reload policy, required reauthentication after legacy cache changes, same-origin/XSS exposure, local-only app sign-out versus provider logout/revocation, failed storage removal, trusted-userinfo verification limits, and manual live-system qualification. See `packages/sdk/README.md`, `packages/oidc-client/README.md`, and `apps/password-manager/README.md`.

Validation passed:

- `pnpm --filter @of/sdk test` (25 workflow tests); OIDC client tests; Storage client tests; existing collection suite (59 tests).
- `pnpm --filter password-manager test` (25 tests, including real SDK redirect/callback/reload and mounted CRUD/Refresh/sign-out workflows).
- App configuration Node tests (2); OIDC/Storage TypeScript builds; SDK build; app production build; app check (0 errors/warnings).
- SDK lint/format, focused changed-source Biome checks, both changed Svelte components' autofixer (no issues/suggestions), and `git diff --check`.
- No Rust changes or compilation. `just --show crap` confirms it invokes Rust workspace coverage/Cargo CRAP; it does not qualify TypeScript and was not executed in the known broken nested Rust-sibling layout.

Scope/qualification boundary: ticket 02 owns SDK-bound Storage discovery/create/open capabilities and retained-connection/late-open handoff guarantees. This slice intentionally preserves the existing app Storage factory with an SDK-captured token provider and synchronous adapter teardown, not a second collection implementation. Raw retained Storage sockets are not independently SDK-guarded yet. No running Installation/provider credentials were available; deployed OAuth/provider logout, replication, and production vault security remain unqualified. No push performed.
