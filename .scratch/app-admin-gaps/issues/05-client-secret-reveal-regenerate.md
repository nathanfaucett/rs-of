# Issue 05: Confidential client - reveal/regenerate secret in detail

## Description

Client detail page (`/clients/[clientId]`) shows metadata but no access to `client_secret`. Add "Reveal secret" (with confirmation) and "Regenerate secret" actions.

## Type

task

## Priority

Medium

## Status

open

## Blocked by

- None for **Reveal** (API verified: `idpApi.getRegister()` returns `ClientRegistration` with `clientSecret`)
- **00-backend-client-secret-rotate**: IdP server must support secret rotation (no dedicated endpoint; `putRegister` may work but untested)

## Acceptance Criteria

- **Reveal secret**: Button → confirmation dialog → modal shows `clientSecret` with copy button → "Done" closes
- **Regenerate secret**: Button → confirmation → calls IdP rotation endpoint → modal shows new `clientSecret` with copy button → old secret invalidated
- Only enabled for confidential clients (`tokenEndpointAuthMethod !== "none"`)

## Seams

- `idpApi.getRegister({ clientId })` — **VERIFIED**: returns `ClientRegistration` with `clientSecret`
- `idpApi.putRegister()` — available for updates; secret rotation via `putRegister` untested
- File: `of/apps/app/src/routes/(auth)/clients/[clientId]/+page.svelte`
- Add actions in "Danger zone" card or new "Credentials" card
- UI: Modal with copy button, confirmation dialogs
- Conditional: only for confidential clients (`tokenEndpointAuthMethod !== "none"`)

## Notes

- **VERIFIED**: @ofsdk/idp-client `getRegister()` returns `ClientRegistration` with `clientSecret`
- **Reveal secret**: Ready to implement (no backend blocker)
- **Regenerate secret**: Requires testing `putRegister` with new secret or backend rotation endpoint
- Current client detail shows: metadata, keys, redirect URIs, scopes, danger zone (delete)
- Secret access should be audited (log when revealed/regenerated) — future enhancement
- Public clients (`tokenEndpointAuthMethod: "none"`, `clientType: "public"`) don't have secrets

## Comments

## Answer
