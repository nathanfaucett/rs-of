# Service Boundary Specification

Status: ready-for-agent

Work status: resolved

## User outcome

An authorized, active User can approve and administer Devices without confusing User identity, endpoint identity, or Resource access. A Device can withdraw its own approval with its endpoint key, without signing in as a User. This work advances [M3](../../docs/goal.md#m3--two-device-offline-use-and-administration); it does not complete M3.

## Scope

- IdP authenticates the request Principal and owns current User status. Management evaluates exact administrative actions at Installation scope. Device owns configuration, endpoint bindings, and lifecycle writes. Storage owns Resource access and Selections in the target design.
- Keep writes on the Device API. For rename, pairing approval, and administrative revocation, require a Device-audience User access token, current active status for the verified User subject, and the exact Management permission. Reuse the owner interfaces for local and hosted deployments. Device enforces proof and state rules before mutation. Do not put cross-owner authorization in `DeviceRepo`.
- Keep `DevicesRename` for rename, `DevicesEnroll` for pairing, and `DevicesRevoke` for administrative revocation. Do not add `DevicesApprove` or change default grants in this feature.
- Approve a pending pairing only with a valid signature over the exact approval payload from the new Device key and a valid accepting Device binding. The legacy enrollment approval route must not approve a pairing without that proof.
- Keep self-revocation separate: verify the endpoint signature and apply Device state rules without User authentication or Management policy. Administrative revocation retains its current Device-state protections.
- Approval does not create a Selection or grant Resource access. Learned revocation blocks later authorized Device use. It does not transfer Resource ownership or erase copies.

## Non-goals and compatibility

No Management Device-write routes, address updates, endpoint-key rotation, shared-Device migration, or changes to the legacy owner/transfer flow. Preserve current route locations, audiences, permission names, signing payloads, and persisted formats. No store or grant migration. The legacy owner field is neither the authenticated actor nor the target shared-Device model.

## Acceptance checks

- [x] Rename requires the exact permission and active actor; denial leaves the name unchanged.
- [x] Signed pairing approval and administrative revocation succeed through the public Device API only when authorization and Device rules pass. Test independent policy, identity, proof, and state failures where applicable. Each denial leaves the record unchanged.
- [x] Signed self-revocation works without IdP or Management. Invalid proof changes no state.
- [x] Approval leaves Selections unchanged, and revocation blocks later use after it is learned.
- [x] Unified routing preserves owner prefixes and token audiences. API descriptions distinguish the User subject, Device ID, and endpoint key.

## Dependencies and current evidence

No external blocker. See [issues](issues/) for the focused checks and real-backend qualification. [Architecture](../../docs/architecture.md) and the [glossary map](../../GLOSSARY-MAP.md) define target ownership. Current handlers already check active Users and exact permissions; current owner tests cover signed pairing and four-owner health. These are a baseline, not proof of all acceptance checks or shared-Device support.

## Comments

Resolved on `service-boundaries-integration`. The shared database, schema, and permission grants are unchanged. Focused Device, model, and IdP tests, the serial unified composition suite, generated Device contract checks, and Playwright owner HTTP tests passed. The parallel run hit the existing startup deadline; the serial run passed. Full browser and workspace suites were not run. The unrestricted IdP feature-powerset run stopped on an existing test configuration error; the supported combinations passed the focused IdP test. No independent deployment or multi-Device convergence qualification is claimed.
