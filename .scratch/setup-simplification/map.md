# Setup simplification map

## Notes

- Delivery follows [the spec](spec.md) and numbered tickets. Each worktree owns its assigned slice; source approval UI is ticket 04.
- Ticket 01 started on `setup-simplification-01` at integration `setup-simplification` / `dfeaf7dbe`; the integration merge was already up to date.

## Decisions-so-far

- [01: Unify onboarding observation and recovery routing](issues/01-unify-onboarding-observation.md) — resolved. All onboarding/authentication routing consumes the target-bound typed observation. Shared worker evidence remains authoritative, remote/browser observation stays direct HTTP, native capability retrieval is separate and admission parsing is shared. Superseded observation commands/functions are deleted. [Validation evidence](evidence/01-onboarding-observation.md).
- Preserve the Client-bound Device preference step in this observation-only slice. Later tickets may remove its dismissal gate without weakening current readiness or changing actual local selection permissions.
- Safe failure actions distinguish transient retry, prerequisites and exact retained-identity repair; destructive reset remains explicit maintenance, not automatic recovery.
- [04: Source approval](issues/04-simplify-source-approval.md) resolved: one Add a device task, existing owner authorization/adapters, exact decisions and retained partial-approval retry. [Focused evidence and validation limits](evidence/04-source-approval.md): 9 rendered/27 owner-client tests pass; Svelte check/autofix clean. Rust lifecycle, feature matrix and CRAP bounded out compiling; broad auth suite is not green.

- [02: Streamline Create](issues/02-streamline-create.md) — resolved. Credentials-only bundled Create, dependency-free default naming with ordinary rename, exact transient-credential recovery, direct verified ordinary sign-in and deferred authenticated Device preferences. Removed dismissal route/command/flag/persistence; actual restrictions, selections and resource authorization are unchanged. [TDD, caller audit and validation evidence](evidence/02-streamline-create.md). This supersedes ticket01’s temporary preservation of the dismissal gate.
- [03: Guided Join](issues/03-streamline-join.md) — resolved. Dedicated Join UI defaults the name, displays protected authenticated Installation evidence, preserves exact recovery/retry/cancellation and selected remote targets, and gates sign-in on current public login. Parent changes only wire/delete Join; 02/05/06 ownership remains intact. [Evidence](evidence/03-streamline-join.md): 23 rendered/181 auth tests and Svelte check pass; shared HTTPS/matrix/CRAP timed out compiling.

## Fog

- Full Cargo Hack matrix is unqualified: Swagger UI's downloaded/cached archive fails its build script. CRAP coverage compilation exceeded the bounded run; no score is available.
- The standalone setup HTTP script retains 15 independently reproduced baseline failures; the focused Vitest state/route and rendered interaction suites pass.
- Real OS trust/elevation/provider profiles, full desktop GUI, WAN and other platforms remain outside injected same-host test evidence.
- Runtime/security lifecycle, full matrix/CRAP, broad frontend failures and real native GUI qualification remain unverified by ticket 04; see its evidence. No observation/startup/setup implementation was changed by ticket 04.
