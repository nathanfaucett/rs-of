# Service Boundary Specification

**Status:** ready-for-agent

## What to build

Define clear ownership boundaries for the four domain services (IdP, Management, Device, Storage) in the elder-coral project, addressing:

1. **Identity split**: Clarify that "Identity" is split into user identity (OAuth/OIDC, IdP-service) and device endpoint identity (Iroh keys, Device-service).
2. **Configuration ownership**: All device configuration (name, address, endpoint identity) must be set through the Management API, not directly via Device-service.
3. **Approval/revocation coordination**: Device approval/revocation requires three-layer enforcement: Management policy evaluation, IdP active user check, and Device-service state mutation.

## Acceptance criteria

- [ ] Device-service routes for config changes require Management policy evaluation
- [ ] Device approval/revocation enforces Management policy + IdP active user check + Device-service state mutation
- [ ] Documentation clarifies the identity split
- [ ] unified-server composition respects ownership boundaries
- [ ] End-to-end device approval flow passes with full three-layer enforcement

## Blocked by

None (can start immediately)

## Dependencies

- `services/` domain services
- `docs/architecture.md`
- `docs/goal.md`
- `GLOSSARY-MAP.md`

## Current evidence or qualification limits

- Current code shows boundary leaks: device-service accepts config changes directly; approval/revocation is distributed across services without clear single-owner enforcement; identity split is not documented at API layer.
- Current support: local unified backend available; target architecture not fully implemented.

## Comments

- See `.scratch/service-boundaries/issues/` for the tracer-bullet ticket breakdown.
