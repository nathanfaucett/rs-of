# Issue 00: Backend - Device approval for admin-initiated enrollment

## Description

Add explicit device approval endpoint for admin-initiated enrollments. `deviceApi.enrollDevice()` returns `state: "pending"` but no approve endpoint exists in `@ofsdk/device-client` DefaultApi.

## Type

task

## Priority

Critical

## Status

open

## Blocked by

- None

## Acceptance Criteria

- POST `/devices/enrollments/{id}/approve` or similar in Device server
- Admin calls to approve pending enrollment
- Returns updated `DeviceEnrollment` with `state: "approved"`
- Requires admin scope: `device.enrollments.approve`
- Generated client `@ofsdk/device-client` includes `approveEnrollment` method

## Seams

- Device server: enrollment routes
- OpenAPI spec update → regenerates `@ofsdk/device-client`
- May relate to `IndependentJoinApi.independentJoinApprove()` — investigate if that's the intended flow

## Notes

- Blocks: 02-device-enrollment (admin approval step)
- `enrollDevice` exists and returns `{id, state: "pending"|"approved", approvalCode?}`
- Need to clarify: does `enrollDevice` auto-approve for some cases? Or always requires approval?
- Check if `IndependentJoinApi.independentJoinApprove()` is the correct approval mechanism

## Comments

## Answer
