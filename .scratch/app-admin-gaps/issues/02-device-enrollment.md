# Issue 02: Device enrollment flow

## Description

Add device registration from the admin UI. Currently `/devices` only lists already-enrolled devices. Admin needs to register new devices: generate keypair, show QR code for pairing, approve enrollment.

## Type

task

## Priority

Critical

## Status

open

## Blocked by

- **00-backend-device-approve**: Device server must add approval endpoint for admin-initiated enrollments (enrollDevice returns `state: "pending"` but no approve endpoint in @ofsdk/device-client)

## Acceptance Criteria

- "Add device" button on `/devices` page
- Generates device keypair (Ed25519)
- Shows QR code with `device://` URI containing public key
- Admin approves → device appears in list with "pending" state
- Device can then sync (auto-refresh picks it up)

## Seams

- `deviceApi` — **AVAILABLE**: `enrollDevice({address, name, publicKey})` → returns `{id, state, approvalCode?}`
- `deviceApi` — **MISSING**: explicit approve endpoint for admin-initiated enrollments
- Route: `(auth)/devices/+page.svelte` → add "Add device" button + modal
- QR code generation: `qrcode` package (already in deps)
- UI patterns: Page, Card, Field, Alert, notifications
- Need to investigate: does `enrollDevice` auto-approve or require `IndependentJoinApi.independentJoinApprove()`?

## Notes

- **Verified**: `enrollDevice` exists in @ofsdk/device-client, takes `address`, `name`, `publicKey` (Ed25519)
- Returns `DeviceEnrollment` with `id`, `state` ("pending"|"approved"|"revoked"), `approvalCode?`
- **MISSING**: No explicit approve endpoint in DefaultApi — may need IndependentJoinApi
- Current flow: user enrolls via CLI/device app, admin sees it in list
- This ticket adds admin-initiated enrollment (reverse flow)

## Comments

## Answer
