# Setup simplification map

## Notes

Work is split among independent tickets; this entry records only source approval.

## Decisions-so-far

- [04: Source approval](issues/04-simplify-source-approval.md) resolved: one Add a device task, existing owner authorization/adapters, exact decisions and retained partial-approval retry. [Focused evidence and validation limits](evidence/04-source-approval.md): 9 rendered/27 owner-client tests pass; Svelte check/autofix clean. Rust lifecycle, feature matrix and CRAP bounded out compiling; broad auth suite is not green.

## Fog

- Runtime/security lifecycle, full matrix/CRAP, broad frontend failures and real native GUI qualification remain unverified by ticket 04; see its evidence. No observation/startup/setup implementation was changed here.
