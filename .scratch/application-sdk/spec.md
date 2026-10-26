# Application SDK: Authenticated Storage Composition

Status: resolved

## Problem Statement

The system provides OIDC, IdP, and Storage clients and has a planned TanStack DB Storage integration, but application developers still have to assemble authentication, configuration, token access, resource access, and session cleanup themselves. The password manager currently distributes this wiring across application state modules and routes. Copying that wiring into another application would also copy its lifecycle and isolation risks.

The User wants a small SDK that brings these capabilities together and makes the password manager the first real reference application. The SDK must remove integration work without merging domain ownership, introducing a competing state framework, or hiding unsupported guarantees.

## Solution

Add a framework-independent `@of/sdk` workspace package that composes the existing OIDC and Storage clients into a small authenticated application interface. Migrate the password manager in the same delivery so the package earns its interface through a working consumer, not speculative abstractions.

The SDK owns authenticated-context coordination and supplies context-bound Storage access. Existing clients retain protocol ownership. The Storage collection adapter retains record persistence, reconciliation, and connection lifecycle ownership. Official TanStack Svelte bindings retain reactive query integration. The application retains navigation, resource selection, schemas, business rules, and UI state.

This follows completion of the TanStack DB integration effort. Do not duplicate or reopen that effort's collection implementation, tests, or scope.

## User Stories

1. As an application developer, I want to configure one SDK instance, so that I do not assemble authentication and Storage token wiring in every application.
2. As an application developer, I want explicit IdP and Storage service bases, so that independently hosted services and path prefixes work correctly.
3. As an application developer, I want Application metadata and OAuth Client configuration to remain distinct, so that SDK convenience does not change domain identity.
4. As an application developer, I want the existing sign-in and registration behavior reused, so that the SDK does not implement another OAuth stack.
5. As a User, I want to sign in through the configured IdP, so that I can access my Application's resources.
6. As a User, I want the redirect callback to establish the correct authenticated context, so that returning from sign-in does not lose or confuse my identity.
7. As a User, I want session restoration to follow a documented policy, so that reloading the example behaves predictably.
8. As an application developer, I want access-token renewal delegated to the existing OIDC client, so that expiry handling is not duplicated.
9. As an application developer, I want Storage token exchange delegated to the existing Storage client, so that authorization actions and audience remain correct.
10. As a User, I want to discover filesystem Storage Resources, so that I can choose a resource in my User/Application namespace.
11. As a User, I want to create a filesystem, so that a fresh account can use the example without manual provisioning.
12. As a User, I want resources opened by Filesystem ID, so that duplicate display names cannot select the wrong resource.
13. As an application developer, I want SDK-backed Storage access to work with the existing collection adapter, so that record behavior has one implementation.
14. As a User, I want the password manager's existing CRUD and explicit Refresh workflow preserved, so that SDK adoption does not remove working functionality.
15. As a User, I want local sign-out to clear visible secrets even if the IdP is unreachable, so that a network failure does not leave the previous session usable.
16. As an application developer, I want local session termination distinguished from provider logout and token revocation, so that I do not promise security effects the SDK cannot establish.
17. As a User, I want changing IdP, Application/client configuration, or User to discard obsolete authenticated data, so that another context cannot inherit my secrets.
18. As an application developer, I want old context handles to become unusable after termination, so that retained references cannot initiate new work with another User's token.
19. As a User, I want late callback, token, discovery, and filesystem results isolated from my current context, so that racing requests cannot restore old data.
20. As an application developer, I want token renewal within the same identity not to reset collections unnecessarily, so that normal expiry handling does not interrupt editing.
21. As an application developer, I want small framework-neutral session notifications, so that my application can clear context-bound views without an SDK-specific reactive framework.
22. As a Svelte developer, I want to keep official TanStack Svelte bindings, so that adopting the SDK does not require another custom Svelte wrapper.
23. As a User, I want useful authentication and Storage errors without secret contents or tokens, so that failures are understandable without exposing sensitive data.
24. As an application developer, I want multiple Application/client configurations isolated within one browser origin, so that one application's credentials cannot be reused accidentally by another.
25. As a maintainer, I want clear resource ownership and deterministic teardown, so that every collection and socket has one lifecycle owner.
26. As a maintainer, I want public-workflow tests and a runnable reference application, so that SDK convenience is demonstrated rather than asserted.
27. As an application developer, I want accurate setup and security documentation, so that browser freshness, credential persistence, and Node replication are not confused.

## Implementation Decisions

### Delivery and dependency direction

- Deliver after the approved TanStack DB integration is complete. Its initial load, CRUD, explicit Refresh, isolation, and disposal contracts form the baseline. This is a follow-on composition effort, not a replacement collection project.
- Add one framework-independent TypeScript workspace package named `@of/sdk`. The different npm scope from the existing `@aicacia/*` packages is intentional for this proposed public entry point; it does not require renaming existing packages or external registry publication.
- The SDK depends on the existing OIDC and Storage clients. It must not depend on Svelte, SvelteKit, password-manager modules, or the TanStack collection adapter. The collection adapter must not depend on the SDK. The application composes SDK-provided context-bound Storage access with the adapter's existing Storage seam.
- Do not add a generic provider interface, plugin registry, dependency-injection container, or pass-through copy of every client method. The SDK earns its depth by eliminating repeated configuration, token wiring, and context lifecycle coordination.
- Implement and migrate the password manager together. Delete superseded application composition rather than leave a second auth-to-Storage route or compatibility facade.

### Configuration and protocol ownership

- Accept explicit IdP and Storage service bases, OAuth Client configuration, Application registration metadata, redirect URLs, and the existing supported transport configuration needed by this browser application. Do not read framework environment modules or infer Storage from the IdP origin. Validate service URLs at the owning client seam; keep path prefixes intact and reject embedded credentials or ambiguous URL components.
- Application URI, OAuth Client ID, IdP service base, and discovered canonical issuer are distinct concepts. Do not equate a Node service URL with Installation identity or use a client ID as Application identity. Use the OIDC client's validated discovery/identity contract rather than inventing SDK identity verification.
- Reuse the current project's supported authorization-code/PKCE and registration behavior. App-specific metadata, requested scopes, public-client settings, and redirect destinations are supplied by the application. Do not hard-code the password manager's client ID, resource names, or branding.
- OIDC owns discovery, callback validation, PKCE, token storage, token renewal, user information, and provider logout mechanics. Storage owns Storage-Audience Access Token exchange and filesystem requests. Do not copy either protocol implementation into the SDK.
- Any missing correctness behavior in those clients is fixed in the owning client with focused tests, not patched by decoding credentials or issuing duplicate protocol requests in the SDK. Preserve lower-level clients as independently usable modules.
- The SDK supplies the Storage client's bearer-token provider from the captured authenticated context. Expose only the small context-bound capability required for resource discovery, creation, and connection opening; do not expose mutable raw-client access that bypasses lifecycle checks.

### Authenticated-context lifecycle

- Treat the SDK's authenticated context as a client-runtime concept, not a new domain entity or a Storage Session. It represents one validated issuer/User/Application/client configuration with an instance-local generation for isolation. It is not a permission grant.
- Keep instance configuration immutable. Reconfiguration retires/disposes the previous SDK instance and creates a new one. Do not allow a live resource handle to follow mutable global configuration.
- Establish authenticated state through supported callback/restoration and user-information behavior, not merely the presence of an access-token string or an unverified decoded claim. No-code callbacks, invalid state, or failed identity establishment must not report authenticated success.
- Expose a minimal read-only session snapshot and subscribe/unsubscribe mechanism sufficient for the app to react to authentication changes. Define initialization, unauthenticated, authenticated, and ended/disposed behavior without creating another general-purpose reactive store. Do not include tokens in snapshots or notifications.
- A refresh for the same verified identity retains the context. A changed subject or issuer ends it. Missing credentials and terminal authentication failures require reauthentication; transient connectivity failures and resource authorization denials must not automatically be treated as a different User or erase the session indiscriminately.
- Ending a context invalidates new work through all retained handles before awaiting network operations. Late discovery, callback, renewal, and connection results cannot reauthenticate the ended instance, supply tokens to a new context, or populate another context's views. Close a socket that finishes opening after its context was retired.
- Do not promise cancellation of server work already sent. Distinguish local disposal from remote write cancellation, provider logout, and token revocation. Do not automatically replay ambiguous mutations.
- Local sign-out clears local authentication and context-bound UI independently of provider availability. Provider logout remains delegated and its failure is reported separately; an unreachable provider must not prevent local session termination.
- SDK disposal is idempotent, stops its notifications and new work, and retires its owned contexts. Unsubscribing stops delivery. No automatic polling, background renewal scheduler, or reconnect loop is added.

### Resource ownership and collection composition

- Each resource has one cleanup owner. The SDK owns its clients, authenticated-context coordination, and untransferred connections. The collection adapter owns connections successfully transferred to it and its own collection/query cleanup. The application owns selected Filesystem ID, form/editor state, and its subscriptions.
- On context retirement, notify the application synchronously so its composition disposes the old adapter/collection and clears the editor. Context-bound connection access also stops accepting new requests. Use the adapter's existing disposal operation; do not create an SDK collection registry or a second reconciliation/cleanup engine.
- Specify the connection handoff explicitly: failed or obsolete opens are cleaned up by the SDK; a successfully adopted connection is cleaned up through the adapter. If a minimal context-aware connection guard is necessary, define its request/close behavior once, keep close idempotent, and do not wrap record logic or create competing owners.
- Switching only the Filesystem ID remains application/adapter lifecycle work and does not end the authenticated context. Use stable Filesystem IDs, never resource display names. Discovery is not permission to open a resource.
- Preserve the collection adapter's validation, safe paths, optimistic mutation settlement, authoritative reconciliation, manual refresh, and late-result isolation. The SDK does not implement record serialization, collection identity, mutation handling, or TanStack query helpers.

### Browser security and application responsibilities

- Choose and document credential persistence deliberately using the owning OIDC client. Do not silently switch the redirect application to memory-only storage: a full-page redirect loses in-memory PKCE state. Verify callback continuity and reload behavior against the chosen policy, and document the exposure of persistent browser credentials. Do not introduce a second token cache in the SDK.
- The inspected OIDC cache keys use authority alone. Before qualifying the SDK, ensure credentials and pending authorization transactions are isolated by authority and OAuth Client configuration, with appropriate callback transaction binding. Fix this in the OIDC client; do not put a parallel cache in the SDK. Invalidation must not purge unrelated clients' credentials. Do not retain unsafe cache compatibility behavior merely to preserve old state.
- Never persist decrypted secrets or secret mutations in browser storage. Never put tokens, callback parameters, PKCE verifiers, or secret contents in session notifications, logs, or user-facing diagnostics. Address unsafe diagnostics on the exercised client path at their owner.
- This first consumer is the existing browser-only password manager. Framework-independent does not promise SSR, native transport, or server-side confidential-client support; do not create process-global authenticated instances or claim untested runtimes.
- The app owns environment mapping, configurable IdP selection, safe return-to navigation, route guards, notifications, public URL/base-path handling, and all Secret schema/history/form behavior. Navigation after authentication is an application decision; the existing OIDC authorization navigation is not reimplemented.
- The app continues to use the official Svelte DB bindings directly. Keep one app-level composition point, not an Aicacia Svelte wrapper package. Do not require Management or Device configuration for the ordinary User sign-in-to-Storage workflow.
- Complexity/risk assessment: priority is a reusable, executable application entry point. Highest risks are credential cache collisions, redirect transaction continuity, sign-out failure, cross-context races, and duplicate cleanup owners. Prefer immutable configuration and explicit lifecycle over a generalized session framework.

## Testing Decisions

- Proposed primary seam: exercise the public SDK workflow with the real existing OIDC and Storage clients against controlled IdP/Storage responses and controlled browser navigation/WebSocket behavior. Assert observable state, resource access, errors, and cleanup, not internal method calls or object layout. Keep transport substitutions at existing client/platform seams; do not invent a service-provider abstraction solely for tests.
- Add a focused password-manager smoke path using the real SDK, collection adapter, and TanStack runtime with controlled service responses. Verify sign-in/callback, filesystem discovery/create/open, CRUD, reload persistence according to the documented credential policy, explicit Refresh, filesystem switching, and sign-out clearing data.
- The testing seam is proposed, not yet User-confirmed. Ask for confirmation when presenting this spec; publication does not imply approval of that detail.
- Verify distinct IdP/Storage prefixes, Application/client metadata propagation, missing/malformed configuration, callback errors and invalid/no-op callbacks, token expiry/renewal, failed user-information establishment, and denied Storage access. Preserve client-specific protocol tests rather than duplicate their entire suites at SDK level.
- Verify local sign-out with failed IdP discovery/provider logout, disposal during callback and refresh, stale list/open completion, old handles after context retirement, and a new sign-in while old work is outstanding. No old work may revive a session or mutate a new context's views. Already-sent writes are not asserted to be canceled.
- Verify two OAuth Clients on the same authority cannot share cached credentials or PKCE transactions, and clearing one does not clear the other. Exercise full-page redirect continuity; a same-object callback test alone cannot establish it.
- Verify adapter-owned connection cleanup, obsolete-open cleanup, idempotent disposal, unsubscribe behavior, and absence of new work after retirement. Confirm normal same-identity token renewal does not unnecessarily destroy open collections.
- Prior art includes the OIDC client's PKCE, token, discovery, error, and memory-storage tests; Storage client's service-prefix/token-exchange/socket tests; password-manager configuration tests; and the completed TanStack integration's public collection and smoke tests. Extend or replace tests where responsibility moves; do not keep redundant tests of deleted app wiring.
- Run applicable package tests, TypeScript checks, Svelte checks, lint/format checks, production build, and available complexity validation. Report unavailable tooling and pre-existing failures. Rust feature-powerset validation is required only if Rust changes become necessary, not as a substitute for TypeScript validation.
- Document and execute a real-system qualification path where available. Controlled responses are not proof of provider logout, Node replication, cross-Node authorization convergence, or production password-vault security.

## Out of Scope

- Implementing the prerequisite TanStack DB integration as part of this SDK effort or modifying its existing planning documents.
- A custom Svelte SDK package, UI toolkit, route framework, TanStack wrapper, or generic application framework.
- New OAuth flows, a second token manager, SDK-owned protocol discovery/verification, or a new authentication backend.
- Management administration, Device enrollment, Installation Setup, resource sharing, or changes to User/Application ownership.
- Generic multi-provider plugins, automatic endpoint discovery, cross-tab session synchronization, or supporting several concurrently active accounts in one SDK instance.
- Automatic reconnect, polling, browser change feeds, background synchronization, offline mutation queues, or new atomic-write guarantees.
- New password-manager features, UI redesign, browser vault encryption, or claims of end-to-end encryption.
- SSR/native/server-runtime qualification, migrating other applications, renaming existing npm scopes, or external package publication.

## Further Notes

- The earlier decision against a custom Svelte wrapper still stands. A framework-independent SDK addresses authenticated composition, not Svelte reactivity; it must not absorb the collection adapter or official bindings.
- No exact SDK class/function names or unverified method shapes are prescribed. Inspect the completed adapter and current clients when implementing, then choose the smallest interface that supports the reference workflow and lifecycle invariants above.
- Current inspection found OIDC sign-out discovers the provider before clearing tokens, authority-only credential/PKCE cache prefixes, a memory-storage mode that cannot itself survive a redirect, and no authentication-change event in the existing event set. These are concrete integration constraints, not evidence that a new OAuth implementation is needed.
- Storage-Audience Access Tokens remain ordinary IdP access tokens subject to Storage authorization. A context handle, resource catalog entry, or open transport never replaces server-side authorization.
- Node replication, browser collection freshness, local credential deletion, remote logout, and remote token validity are separate properties. Documentation must describe only the behavior actually verified.
