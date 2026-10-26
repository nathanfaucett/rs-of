# Application SDK — Map

## Notes

- [Specification](spec.md) — Status: ready-for-agent.
- Prerequisite: complete the [TanStack DB integration](../tanstack-db-integration/map.md) before implementing this follow-on effort.
- [01: Sign in and out through @of/sdk](issues/01-sign-in-and-out-through-sdk.md) — resolved; see Answer for implementation and checks.
- [02: Use SDK-bound Storage in the password manager](issues/02-use-sdk-bound-storage.md) — ready-for-agent; unblocked by resolved 01.
- TanStack DB prerequisite completed at `f16091834`. Implementation branch: `integration/application-sdk`; ticket 01 is resolved; ticket 02 is the current frontier.

## Decisions-so-far

- One new framework-independent `@of/sdk` composes the existing OIDC and Storage clients; migrate the password manager with it.
- SDK owns authenticated-context coordination; existing clients own protocols; the collection adapter owns records, reconciliation, and adopted connections; the app owns navigation, selection, schemas, and forms.
- No SDK-to-collection dependency, reverse dependency, custom Svelte package, protocol duplication, or generalized application framework.
- Configuration is immutable per instance; retired context handles cannot follow a new User or IdP. Define connection handoff and use existing adapter disposal rather than invent a second cleanup registry.
- Fix client correctness gaps in their owning packages. Authority-only credential keys, redirect continuity, provider-logout failure, and overly broad credential purges require explicit qualification.
- Preserve the prerequisite's explicit Refresh model and security limitations.
- The User approved two vertical slices: authentication including safe sign-out, then SDK-bound Storage and the full reference workflow. Tests, documentation, lifecycle safety, and qualification ship with the behavior, not separate scaffolding or hardening tickets.

- [Ticket 01 resolved](issues/01-sign-in-and-out-through-sdk.md#answer): real SDK sign-in/callback/restoration, immutable configuration, verified userinfo identity, local-first sign-out, app migration, and lifecycle isolation delivered together. `@of/sdk` is workspace-only; no publication.
- OIDC owns client/configuration-scoped localStorage credentials and PKCE, preserving full-page redirect/reload continuity. Legacy authority-only caches are ignored; clearing never purges unrelated clients. Plaintext browser credential/XSS exposure is documented.
- Concurrent renewal is coalesced in OIDC. Callback attempts retire prior SDK generations before awaiting network/identity, preventing late userinfo failures from retiring a newer session. Local sign-out notifies/clears the app before provider work; provider failure and persistent-removal failure do not restore visible authentication.
- The app's IdP preference is synchronously persisted and instance-local (no cross-tab auto-reconfiguration). The old direct IdP/OIDC composition is deleted. The existing collection adapter and official Svelte binding remain unchanged.
- Ticket 01's transitional Storage factory captures the SDK token provider; ticket 02 replaces that composition with bounded Storage capabilities/connection guards and qualifies handoff. Do not claim those later guarantees for raw retained sockets now.

## Fog

- User approved the public-SDK workflow test seam: real OIDC/Storage clients and TanStack runtime against controlled service/platform responses, with a focused app smoke test. Live-system qualification remains separate.
- Verify the completed adapter's exact connection/disposal interface before defining SDK interface names or resource handoff types.
- Registry availability for `@of/sdk` is not established; this effort only requires a workspace package.

- Ticket 01 controlled SDK/app qualification passes; live deployed provider/logout/replication qualification remains manual. See the SDK and password-manager READMEs.
