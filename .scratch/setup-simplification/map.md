# Setup simplification map

## Notes

- Delivery follows [the spec](spec.md) and numbered tickets. Each worktree owns its assigned slice; source approval UI is ticket 04.
- Ticket 01 started on `setup-simplification-01` at integration `setup-simplification` / `dfeaf7dbe`; the integration merge was already up to date.

## Decisions-so-far

- [01: Unify onboarding observation and recovery routing](issues/01-unify-onboarding-observation.md) — resolved. All onboarding/authentication routing consumes the target-bound typed observation. Shared worker evidence remains authoritative, remote/browser observation stays direct HTTP, native capability retrieval is separate and admission parsing is shared. Superseded observation commands/functions are deleted. [Validation evidence](evidence/01-onboarding-observation.md).
- Preserve the Client-bound Device preference step in this observation-only slice. Later tickets may remove its dismissal gate without weakening current readiness or changing actual local selection permissions.
- Safe failure actions distinguish transient retry, prerequisites and exact retained-identity repair; destructive reset remains explicit maintenance, not automatic recovery.
- [04: Source approval](issues/04-simplify-source-approval.md) resolved: one Add a device task, existing owner authorization/adapters, exact decisions and retained partial-approval retry. [Focused evidence and validation limits](evidence/04-source-approval.md): 9 rendered/27 owner-client tests pass; Svelte check/autofix clean. Rust lifecycle, feature matrix and CRAP bounded out compiling; broad auth suite is not green.

## Fog

- Full Cargo Hack matrix is unqualified: Swagger UI's downloaded/cached archive fails its build script. CRAP coverage compilation exceeded the bounded run; no score is available.
- The standalone setup HTTP script retains 15 independently reproduced baseline failures; the focused Vitest state/route and rendered interaction suites pass.
- Real OS trust/elevation/provider profiles, full desktop GUI, WAN and other platforms remain outside injected same-host test evidence.
- Runtime/security lifecycle, full matrix/CRAP, broad frontend failures and real native GUI qualification remain unverified by ticket 04; see its evidence. No observation/startup/setup implementation was changed by ticket 04.
