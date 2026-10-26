# Application SDK — Map

## Notes

- [Specification](spec.md) — Status: ready-for-agent.
- Prerequisite: complete the [TanStack DB integration](../tanstack-db-integration/map.md) before implementing this follow-on effort.
- [01: Sign in and out through @of/sdk](issues/01-sign-in-and-out-through-sdk.md) — ready-for-agent; blocked by completion of TanStack DB integration tickets 02–04.
- [02: Use SDK-bound Storage in the password manager](issues/02-use-sdk-bound-storage.md) — ready-for-agent; blocked by 01.
- TanStack DB prerequisite completed at `f16091834`. Implementation branch: `integration/application-sdk`; ticket 01 is the current frontier.

## Decisions-so-far

- One new framework-independent `@of/sdk` composes the existing OIDC and Storage clients; migrate the password manager with it.
- SDK owns authenticated-context coordination; existing clients own protocols; the collection adapter owns records, reconciliation, and adopted connections; the app owns navigation, selection, schemas, and forms.
- No SDK-to-collection dependency, reverse dependency, custom Svelte package, protocol duplication, or generalized application framework.
- Configuration is immutable per instance; retired context handles cannot follow a new User or IdP. Define connection handoff and use existing adapter disposal rather than invent a second cleanup registry.
- Fix client correctness gaps in their owning packages. Authority-only credential keys, redirect continuity, provider-logout failure, and overly broad credential purges require explicit qualification.
- Preserve the prerequisite's explicit Refresh model and security limitations.
- The User approved two vertical slices: authentication including safe sign-out, then SDK-bound Storage and the full reference workflow. Tests, documentation, lifecycle safety, and qualification ship with the behavior, not separate scaffolding or hardening tickets.

## Fog

- User approved the public-SDK workflow test seam: real OIDC/Storage clients and TanStack runtime against controlled service/platform responses, with a focused app smoke test. Live-system qualification remains separate.
- Verify the completed adapter's exact connection/disposal interface before defining SDK interface names or resource handoff types.
- Choose and document browser credential persistence and callback continuity; memory-only state does not survive full-page redirects.
- Verify canonical issuer/User/Application identity establishment, client-scoped cache behavior, and terminal versus transient failure classification in the current OIDC client.
- Registry availability for `@of/sdk` is not established; this effort only requires a workspace package.
