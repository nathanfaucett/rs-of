# Issue 00: Backend - IdP createUser endpoint

## Description

Add `createUser` admin endpoint to the IdP server. Currently missing from `@ofsdk/idp-client` generated client. Required for admin UI user creation.

## Type

task

## Priority

Critical

## Status

open

## Blocked by

- None

## Acceptance Criteria

- POST `/users` endpoint in IdP server
- Accepts: email, username, password (or invite flow), initial roles
- Returns: created user with `sub`, `email`, `username`, `name`, etc.
- Requires admin scope: `idp.users.create`
- Generated client `@ofsdk/idp-client` includes `createUser` method

## Seams

- IdP server: `crates/idp-server/src/router/routes/users.rs` (or similar)
- OpenAPI spec update → regenerates `@ofsdk/idp-client`
- Admin scope/permission model

## Notes

- Blocks: 01-user-write-ops (create user functionality)
- Other user endpoints exist: `updateUser`, `deleteUser`, `resetPassword`
- Need to decide: password in request vs. invite email with setup link

## Comments

## Answer
