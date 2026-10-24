# 02: Qualify Create and ordinary use across both deployment layouts

**What to build:** One shared full-Node scenario establishes a fresh Installation, verifies readiness, logs in as the Initial Administrator, and performs an authorized resource operation against both a Unified Host and actual independent-service processes. The scenario consumes public generated clients rather than Unified internals.

**Blocked by:** None (can start immediately).

**Status:** resolved

Claimed on refactor/system-tests-02, base 191962716f3edd7fd0c0ffbd8e8ec6c008511393.

- [x] Establish the smallest shared system-test arrangement outside the Unified implementation, with a shared scenario body and small concrete deployment launch/setup/teardown fixtures. Do not introduce a production crate or generic host abstraction.
- [x] Execute the same behavioral assertions against Unified and the documented supported same-machine independent-service layout. Launch actual supported independent processes, including required database-owner processes or documented owner connections; do not substitute handcrafted child-test servers for full-Node qualification.
- [x] Configure explicit IdP, Management, Device, and Storage addresses, TLS trust, and credentials. Do not assume one origin, fixed ports, or identical prefixes.
- [x] Use each layout's documented Create sequence. Independent IdP, Device, and Storage workers each receive their own admission capability and correlated operation tuple; Management is not a fourth setup worker. Do not invent a common production setup endpoint.
- [x] Poll verified public readiness and correlate Installation/operation evidence across independent workers; an HTTP 202, open listener, or unrelated ready flags cannot satisfy completion.
- [x] After Create, verify ordinary login and an authorized owner/resource operation through generated clients and typed models. Do not inspect private databases or seed rows/completion markers to manufacture success.
- [x] Correct missing or inaccurate supported-operation OpenAPI contracts and regenerate affected clients if needed; do not add handwritten HTTP fallbacks. Keep any deliberate wire-level raw HTTP narrowly scoped.
- [x] Isolate roots and ports, account for process-global credential-provider state, use bounded polling and safe diagnostics, and tear down child processes on success and failure. Explicitly document test-only provider/trust substitutions.
- [x] Name/report the deployment layout for each execution and show the shared scenario passing in both layouts. Record focused validation commands and qualification limits; no CRAP/coverage or feature-powerset tests.

## Coordination

This ticket can proceed independently of ticket 01. Own the new shared scenario and deployment fixtures; coordinate any edits to existing Unified tests rather than overlapping the external-test migration. Replace or remove the corresponding old scenario only once the new coverage is demonstrated.

## Answer

Implemented the root-package opt-in `system_create` integration target and a
test-only CLI launcher, outside `unified-server`. Both actual process layouts
passed the same Create/login/resource scenario in two serial final runs (50.39s
and 52.32s). See [ticket02 report](../ticket02-report.md) for exact validation,
coverage coordination, the earlier full-cost fixture 503, and qualification limits.
No existing Unified tests or production implementation files were changed.
