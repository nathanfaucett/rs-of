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

## Review results

Implemented all four review findings on `implementation/application-sdk-review` in `.scratch/worktrees/application-sdk-review`. Initial HEAD and `integration/application-sdk` were both `ae4d6cc384023a3826062fe62d97efa0cab94cbb`; `git merge-base --is-ancestor ae4d6cc38 HEAD` succeeded. The approved public-client and mounted-app test seams remain unchanged; no TanStack planning was reopened.

- **P1 — exact verified token / expiry:** OIDC now returns the exact userinfo-verified access token with identity; the SDK exchanges that token without another renewal. Refresh drops the obsolete access-token deadline and recalculates it from the new response. The rotating, token-specific userinfo regression first reproduced exchange of unverified `token-4` after verifying `token-3`; it now exchanges only `token-3` and retires a changed subject before any subsequent Storage exchange. A client regression reproduced the retained expired deadline and verifies one renewal with a fresh deadline.
- **P2 — overlapping identity reads:** OIDC discards stale userinfo successes and errors and retries current credentials within the original live context instead of raising terminal missing-auth for an obsolete renewal read. The staggered SDK regression reproduced session retirement after successful same-User renewal. Mounted regressions cover both stale success and stale 401 after renewal and retain the exact session, persisted renewed credentials, socket, collection and unsaved editor.
- **P2 — lifecycle event isolation:** the SDK relays only fresh, payload-free close/error events on an isolated EventTarget; `this`, `target`, `currentTarget` and event paths cannot expose the raw socket, send capability or credential URL. The regression reproduced the raw transport escape and now checks those avenues before/after retirement, listener once/removal behavior and rejection of post-retirement requests. Relay listeners detach on close without changing connection handoff ownership.
- **P2 — app recovery:** after terminal authentication failure, the authenticated page still flushes collection/editor/socket cleanup synchronously and renders an app-owned Sign in again link rather than a blank page. The mounted terminal-401 regression reproduced the missing recovery UI and now verifies cleanup, credential removal and the usable signin destination. The existing signin submission replaces ended SDK instances.

Validation: 43 SDK, 15 OIDC, 21 Storage, 59 collection-adapter and 23 password-manager tests plus 2 app-config Node tests pass, including the mounted full redirect/callback/reload/CRUD/Refresh/sign-out workflow. Workspace dependency builds (including OIDC, Storage, SDK and adapter), OIDC/SDK no-emit TypeScript checks, password-manager production build and Svelte check (0 errors/warnings), focused TypeScript Biome lint, format checks (preserving OIDC's existing two-space style), and Svelte autofixer (no issues/suggestions) pass. The initial fresh-worktree dependency-resolution and missing generated Paraglide failures were resolved by building dependencies/the app. Generated build/install/test artifacts are restored before commit.

Limitations: `just crap` was attempted again and fails at Cargo metadata because the nested worktree cannot find sibling `.scratch/worktrees/ofdb/crates/sql/Cargo.toml`. It is Rust-only; no Rust changes or feature-powerset run. Controlled HTTP/WebSocket/jsdom qualification is not live provider/browser/Installation, logout/revocation or replication qualification. No push.

### Retirement/replacement verification follow-up

The follow-up starts from confirmed `884206d06` ancestry on the same review branch/worktree; `integration/application-sdk` also contained that commit. Verification found that unconditional stale-userinfo retries could read replacement credentials and issue a new userinfo request from a disposed SDK before the outer SDK epoch check.

- Added separate OIDC identity-context invalidation for credential clearing, explicit credential replacement and validated code-callback start; ordinary refresh changes only the token revision. Identity establishment captures its original context and checks it before token acquisition, after awaits and before inspecting stored credentials for a retry. Both stale success and stale error paths stop on retirement/replacement rather than following the replacement namespace.
- Added public-SDK regressions for held userinfo returning success and 401 after old-SDK disposal and a different User authenticating in the same credential namespace. Both first failed with an extra old-transport request. They now assert no further old transport requests, no replacement localStorage reads, unchanged replacement persisted credentials/session, working replacement Storage and successful reload restoration. Existing same-User renewal overlap regressions continue to pass.
- Follow-up validation: 45 SDK, 15 OIDC and 23 password-manager tests pass, including mounted same-User overlap (stale success/401) and the full app workflow. OIDC/SDK no-emit TypeScript checks and focused Biome lint/format pass, preserving existing indentation. No Svelte or Rust implementation changed; prior production-build/Svelte qualification and documented live-system/CRAP limitations remain unchanged. Generated artifacts are restored before commit; no push.

## Fog

- User approved the public-SDK workflow test seam: real OIDC/Storage clients and TanStack runtime against controlled service/platform responses, with a focused app smoke test. Live-system qualification remains separate.
- Storage owner `StorageConnection` is the structural adapter seam: lifecycle event subscription, guarded requests and idempotent close. SDK and adapter remain independent; record behavior is unchanged.
- Registry availability for `@of/sdk` is not established; this effort only requires a workspace package.

- Ticket 01 controlled SDK/app qualification passes; live deployed provider/logout/replication qualification remains manual. See the SDK and password-manager READMEs.

- Ticket 02 qualification: 40 SDK, 14 OIDC, 21 Storage, 59 adapter, 20 app and 2 app-config tests pass; TypeScript/Svelte checks, focused lint/format and production build pass. Full mounted real-client/runtime smoke includes redirect/callback/reload, CRUD/Refresh/switch/sign-out and same-identity renewal retaining editor/socket. Late exchanges/opens/requests and context changes are isolated.
- `just crap` was attempted but nested worktree Cargo metadata cannot find sibling `ofdb/crates/sql`; rerun the Rust-only coverage recipe from the main checkout after integration if needed. No Rust changes. Live deployed service/browser/provider and replication qualification remains manual as documented.
