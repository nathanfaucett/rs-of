# Application SDK — Map

## Notes

- [Specification](spec.md) — Status: ready-for-agent.
- Prerequisite: complete the [TanStack DB integration](../tanstack-db-integration/map.md) before implementing this follow-on effort.
- [01: Sign in and out through @of/sdk](issues/01-sign-in-and-out-through-sdk.md) — resolved; see Answer for implementation and checks.
- [02: Use SDK-bound Storage in the password manager](issues/02-use-sdk-bound-storage.md) — resolved; see Answer for implementation, checks and qualification limits.
- TanStack DB prerequisite completed at `f16091834`. Implementation branch: `integration/application-sdk`; tickets 01 and 02 are resolved; no implementation frontier remains.

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
- [Ticket 02 resolved](issues/02-use-sdk-bound-storage.md#answer): frozen generation-bound list/create/open capabilities now replace the transitional app Storage factory and public raw-token provider. Context guards reject obsolete/queued requests and late results; pending opens abort on retirement. Successful open resolution is the one-owner handoff to adapter cleanup, not a collection registry.

## Fog

- User approved the public-SDK workflow test seam: real OIDC/Storage clients and TanStack runtime against controlled service/platform responses, with a focused app smoke test. Live-system qualification remains separate.
- Storage owner `StorageConnection` is the structural adapter seam: lifecycle event subscription, guarded requests and idempotent close. SDK and adapter remain independent; record behavior is unchanged.
- Registry availability for `@of/sdk` is not established; this effort only requires a workspace package.

- Ticket 01 controlled SDK/app qualification passes; live deployed provider/logout/replication qualification remains manual. See the SDK and password-manager READMEs.

- Ticket 02 qualification: 40 SDK, 14 OIDC, 21 Storage, 59 adapter, 20 app and 2 app-config tests pass; TypeScript/Svelte checks, focused lint/format and production build pass. Full mounted real-client/runtime smoke includes redirect/callback/reload, CRUD/Refresh/switch/sign-out and same-identity renewal retaining editor/socket. Late exchanges/opens/requests and context changes are isolated.
- `just crap` was attempted but nested worktree Cargo metadata cannot find sibling `ofdb/crates/sql`; rerun the Rust-only coverage recipe from the main checkout after integration if needed. No Rust changes. Live deployed service/browser/provider and replication qualification remains manual as documented.
