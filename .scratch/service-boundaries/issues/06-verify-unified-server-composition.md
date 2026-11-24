# 06: Verify owner isolation in unified-server

Status: ready-for-agent

Work status: resolved

## User outcome

A client can use all four APIs on one local host. Sharing a host does not let a client bypass an owner's routing or authorization.

## Scope and compatibility

Extend the current unified-server composition tests. Keep existing prefixes and audiences. The embedded database layout is a local implementation choice, not a requirement for other deployments. No changes to store layout, provisioning, or independent deployment.

## Acceptance checks

- [x] All four owner prefixes serve their published surfaces. An unknown prefix and a representative owner route under the wrong prefix return not found.
- [x] A valid User token with a non-Device audience does not authorize a Device administrative write. A Device-audience token without the exact Management grant also fails. Neither request changes Device state.

## Dependencies and current evidence

None. Unified dispatch, four-owner health, and restart coverage already exist. Health checks alone do not establish authorization isolation. Use the existing composition fixture and share its public-API coverage with issue 07 where practical.
