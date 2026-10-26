# 01: Sign in and out through @of/sdk

**What to build:** The password manager uses one framework-independent SDK instance for configuration, sign-in, callback handling, session restoration, and safe local sign-out, including when the IdP is unreachable. Follow the parent Application SDK specification.

**Blocked by:** TanStack DB integration tickets 02–04 — Open a filesystem through TanStack DB; Save and delete secrets through the collection; Refresh and qualify the example. Complete that effort before starting this follow-on migration.

**Triage:** ready-for-agent

**Status:** claimed

- [ ] Add the framework-independent `@of/sdk` workspace package and migrate the password manager's authentication workflow with it; do not ship standalone scaffolding without a working consumer.
- [ ] Accept immutable explicit IdP/Storage service bases, OAuth Client configuration, Application metadata, and redirect configuration. Preserve service prefixes and distinct Application, client, and issuer identities. Keep environment mapping, navigation, safe return-to handling, route guards, and notifications in the application.
- [ ] Delegate discovery, registration, authorization-code/PKCE, callback validation, token renewal, user information, credential persistence, and provider logout to the existing OIDC client. Fix correctness gaps in that client rather than duplicate protocol logic in the SDK.
- [ ] Isolate cached credentials and pending authorization transactions across OAuth Clients on the same authority. Clearing one context must not purge unrelated credentials; do not preserve unsafe legacy cache compatibility.
- [ ] Establish authenticated state through the supported verified identity workflow, not token presence or unverified decoded claims. Invalid/no-op callbacks cannot report authenticated success. Document and test credential persistence, full-page redirect continuity, and reload restoration; memory-only PKCE state is not sufficient for a full-page redirect.
- [ ] Expose only a minimal token-free read-only session snapshot and subscribe/unsubscribe mechanism. Same-identity renewal retains the context; ending or replacing the context invalidates old work before network waits. Transient connectivity errors and resource denials must not indiscriminately end authentication.
- [ ] Local sign-out clears local credentials and authenticated app state even when provider discovery/logout fails; report provider failure separately without claiming revocation. Disposal is idempotent, stops new work, and prevents late callback/renewal results from reviving a retired context.
- [ ] Delete superseded app authentication composition without compatibility facades. Keep the existing collection-backed app usable during this slice; retained Storage wiring must observe the SDK lifecycle so sign-out clears old collections and editor state from the outset.
- [ ] Test public SDK workflows using real OIDC behavior with controlled service/platform responses: sign-in/callback, invalid state, restoration, same-authority client isolation, expiry/renewal, sign-out with an unreachable provider, unsubscribe, and disposal during authentication. Do not log credentials, callback parameters, PKCE verifiers, or secrets.
- [ ] Run relevant package and app checks and update workflow/security documentation. No custom Svelte package, second token cache, background auth scheduler, or new OAuth flow is introduced.
