# Issue 00: Backend - Client secret rotation endpoint

## Description

Add dedicated client secret rotation endpoint (or verify `putRegister` works for rotation). Currently no rotation endpoint in `@ofsdk/idp-client`; `putRegister` may work but untested.

## Type

task

## Priority

Medium

## Status

open

## Blocked by

- None

## Acceptance Criteria

- POST `/clients/{clientId}/rotate-secret` in IdP server
- Returns new `clientSecret`, invalidates old secret
- `clientSecretExpiresAt` updated if applicable
- Requires admin scope: `idp.clients.rotate-secret`
- Generated client `@ofsdk/idp-client` includes `rotateClientSecret` method

## Seams

- IdP server: client registration routes
- OpenAPI spec update → regenerates `@ofsdk/idp-client`
- Alternative: verify `putRegister` with new `clientSecret` works (may require current secret auth)

## Notes

- Blocks: 05-client-secret-reveal-regenerate (regenerate secret only; reveal works without this)
- `getRegister` and `register` both return `clientSecret` — reveal is ready
- `putRegister` exists for client updates; test if it accepts `clientSecret` for rotation
- Public clients (`tokenEndpointAuthMethod: "none"`) don't have secrets

## Comments

## Answer
