# 01: Unify onboarding observation and recovery routing

**What to build:** Users see the correct setup screen and next action from one coherent typed onboarding observation rather than fragmented status reads. This initial prefactoring slice replaces existing observation and routing end to end while preserving current provisioning behavior.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] Trace current setup and authentication-routing callers against the shared worker contract; document required inputs, target selection, and the action/failure table in current topic documentation.
- [ ] Present starting, needs setup, working, ready, and blocked states through one typed snapshot. This is a projection of authoritative evidence, not another persisted lifecycle or provisioning journal.
- [ ] Include enough selected-target identity and safe failure/action information to render the screen without independently reconstructing lifecycle state. Keep bundled-host failure distinct from remotely selected service state.
- [ ] Preserve current verified owner readiness and public login evidence requirements; acceptance, admission, listener availability, sign-in, and historical markers cannot manufacture readiness.
- [ ] Keep privileged setup capability retrieval separate from observation. Move duplicated admission-format parsing behind its owning shared module interface without exposing secrets in status, logs, URLs, or frontend persistence.
- [ ] Use typed shared responses rather than converting them to untyped JSON solely to inspect readiness fields. Browser/remote observation continues through supported interfaces without making Tauri a prerequisite or proxy for all remote operations.
- [ ] Route transient failures to retry, prerequisite failures to actionable instructions, and damaged retained identity to repair; reset is not the default recovery action.
- [ ] Replace existing consumers and delete superseded observation commands and duplicate state logic in this slice; retain commands still required for distinct protected operations. Do not add compatibility wrappers.
- [ ] Verify externally visible setup and authentication routing using existing rendered-page and HTTP/native-adapter test seams, including malformed evidence, target changes, and local failure with remote selection.
- [ ] Update relevant interface and native handoff documentation and run focused tests/checks; record outcomes and qualification limits.
