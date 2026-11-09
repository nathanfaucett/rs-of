# Issue 03: Admin dashboard overview

## Description

Add an overview dashboard at `/` (under `(auth)` layout) showing key stats and quick actions.

## Type

task

## Priority

Low

## Status

open

## Blocked by

- None (aggregates existing API calls)

## Acceptance Criteria

- Route: `(auth)/+page.svelte` (new)
- Stats cards: total users, applications, clients, devices, active devices
- Health indicators: IdP reachable, Management reachable, Device reachable
- Quick action buttons: "Add user", "Add application", "Add device"
- Recent activity (optional): last 5 created users/apps/clients/devices

## Seams

- Existing APIs: `idpApi.listUsers`, `idpAdministration.listApplications`, `idpApi.listClients` (per app), `listDevices`
- Health: reuse `validateIdpApiUrl`, `validateManagementApiUrl`, `validateDeviceApiUrl`
- Layout: `(auth)/+layout.svelte` already provides auth guard

## Notes

- Low priority — core CRUD works without it
- Good "landing page" after sign-in

## Comments

## Answer
