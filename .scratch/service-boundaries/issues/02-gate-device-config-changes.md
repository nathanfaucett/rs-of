# 02: Verify authorization for Device rename

Status: ready-for-agent

## User outcome

An authorized User can rename a Device through the Device API. An unauthorized request cannot change the name.

## Scope and compatibility

Only the existing rename operation. Device owns the write; IdP checks current User status and Management evaluates `DevicesRename`. Preserve the HTTP contract and current Device state and legacy owner checks. No Management write route, owner transfer, address update, or endpoint-key change.

## Acceptance checks

- [ ] A Device-audience token for the active User with the exact Installation permission succeeds for an eligible Device.
- [ ] An invalid or wrong-audience token, inactive actor, missing grant, unrelated grant, or ineligible Device yields the applicable contract error. Read the Device after each denial and confirm that its name did not change.

## Dependencies and current evidence

None. `update_device` already calls the shared permission check. Extend the existing Device authority tests only for missing cases.
