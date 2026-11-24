# 07: Qualify Device approval and revocation through the public API

Status: ready-for-agent

Work status: resolved

## User outcome

An active authorized User can admit a second Device and later withdraw its approval. Device approval alone does not authorize Resource replication. This advances [M3](../../../docs/goal.md#m3--two-device-offline-use-and-administration).

## Scope and compatibility

Extend the real-backend owner tests through unified-server. Use OIDC sign-in and a Device-audience token, real owner state, and explicit fixture grants. Keep the current pairing, revocation, and Selection contracts. No automatic Selection, shared-Device migration, remote erasure, or global revocation guarantee.

## Acceptance checks

- [x] Request a pairing against an approved Device. Check that it is pending. Read the payload as a string, sign its exact UTF-8 bytes with the new Device key, and submit the base64url signature without padding. Verify that the same record becomes approved.
- [x] Independently test missing authentication, an active User without the exact grant, an inactive User who already obtained a token and has the grant, missing or invalid key proof, unsigned approval through the legacy enrollment route, and invalid or repeated state transitions. Confirm that denial leaves state unchanged. Use focused tests from issues 03 and 04 to isolate owner failure cases that the real token path rejects earlier.
- [x] Confirm approval creates no Selection and does not permit replication without separate authorization and Selection.
- [x] Exercise administrative revocation with `DevicesRevoke` and endpoint-signed self-revocation without User credentials. Verify later approved-Device use is denied after the revocation is learned by the tested backend. Do not expect Resource deletion or immediate denial on unaware peers.

## Dependencies and current evidence

Can start against current routes. Reuse issues 01, 03, 04, and 06 for focused assertions rather than duplicating every case. `e2e/owners.e2e.ts` already exercises signed pairing through real owners, but types the approval payload as `number[]`; the generated contract defines a string. Use the generated payload type. Record the real-backend checks run and their qualification limits.
