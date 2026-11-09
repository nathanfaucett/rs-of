# Issue 04: Confidential client - show secret on creation

## Description

When admin creates a client in Application detail, the `client_secret` returned by `idpApi.register()` is discarded. Show it once in a modal with copy buttons.

## Type

task

## Priority

Medium

## Status

open

## Blocked by

- None (API verified: `idpApi.register()` returns `ClientRegistration` with `clientSecret`)

## Acceptance Criteria

- In `applications/[applicationId]/+page.svelte`, `onCreateClient()` captures full response
- Modal displays: `clientId`, `clientSecret` (with copy buttons)
- Warning: "Secret shown once. Store it securely."
- "Copy client ID", "Copy secret" buttons
- "Done" button closes modal
- Secret never shown again in UI unless regenerated (ticket 05)

## Seams

- `idpApi.register()` — **VERIFIED**: returns `ClientRegistration` with `clientSecret` field
- File: `of/apps/app/src/routes/(auth)/applications/[applicationId]/+page.svelte`
- Function: `onCreateClient()` (around line 440-477)
- UI: Modal component (can reuse Alert/Card pattern)
- Conditional: only show secret for confidential clients (`tokenEndpointAuthMethod !== "none"`)

## Notes

- **VERIFIED**: @ofsdk/idp-client `register()` returns `ClientRegistration` with `clientSecret`
- Current code discards response: `const created = await idpApi.register({...})` → only uses `created.clientId`
- Public clients (`tokenEndpointAuthMethod: "none") don't need secret — only show for confidential clients
- Check `clientType` and `tokenEndpointAuthMethod` to conditionally display
- **Ready to implement** — no backend blockers

## Comments

## Answer
