# 01: Verify Device permission contracts

Status: ready-for-agent

Work status: resolved

## User outcome

A User can administer a Device only with the permission for that operation. Advancing [M3](../../../docs/goal.md#m3--two-device-offline-use-and-administration).

## Scope and compatibility

Keep the shared `IdentityAction` model and `ManagementService::evaluate_permission()`. Use existing `DevicesEnroll` for requesting pairings, reading their approval payloads, and approving pairings; `DevicesRename` for rename; and `DevicesRevoke` for administrative revocation. No new permission, default grant, client contract, or store migration.

## Acceptance checks

- [x] Existing tests or focused additions show that Management evaluates each action only at Installation scope, with a valid Device target. Collection targets are allowed for enrollment requests, not rename or revocation. Pairing approval must supply the exact non-nil Device ID at its call site.
- [x] For each operation, a grant for another action does not authorize it. An Application-scoped grant does not confer Installation authority.

## Dependencies and current evidence

None. The actions and evaluator already exist. Management validates the target shape and checks Installation-scope grants; a Device ID in the request does not make the grant per-Device. Reuse existing tests where they cover the checks. Arrange explicit grants in fixtures; do not change setup policy to make tests pass.
