# Host state consolidation

## Notes

Implementation tickets and acceptance scope are in [the specification](spec.md) and `issues/`. Validation is focused; no full feature-powerset, CRAP or live provider/trust qualification is required.

## Decisions-so-far

- [Ticket 01](issues/01-secure-endpoint-identity.md): endpoint secrets are provider-owned; integrated into the host SQL branch from `e36ed4ac2` by merge `53cbddfc3`.
- [Ticket 02](issues/02-host-create-sql.md): resolved. Private host SQL owns configuration, Create workflow and admission retirement; temporary code contains credential material only. Exact lifecycle, transaction, interruption, provider, independent-admission and native reset/reopen evidence and validation limits are recorded in the ticket. Missing SQL requires exact operator backup, not reference reconstruction.

## Fog

- Join-specific persistence consolidation remains ticket 03; it was not implemented in ticket 02.
- Combined HTTPS suite timed out; affected individual wrappers pass. Strict dependency-inclusive lint remains blocked by existing generated-client diagnostics. No live provider/reboot or OS trust certification is claimed.
