# 01: Unify onboarding observation and recovery routing

**What to build:** Users see the correct setup screen and next action from one coherent typed onboarding observation rather than fragmented status reads. This initial prefactoring slice replaces existing observation and routing end to end while preserving current provisioning behavior.

**Blocked by:** None (can start immediately).

Status: resolved

- [x] Trace current setup and authentication-routing callers against the shared worker contract; document required inputs, target selection, and the action/failure table in current topic documentation.
- [x] Present starting, needs setup, working, ready, and blocked states through one typed snapshot. This is a projection of authoritative evidence, not another persisted lifecycle or provisioning journal.
- [x] Include enough selected-target identity and safe failure/action information to render the screen without independently reconstructing lifecycle state. Keep bundled-host failure distinct from remotely selected service state.
- [x] Preserve current verified owner readiness and public login evidence requirements; acceptance, admission, listener availability, sign-in, and historical markers cannot manufacture readiness.
- [x] Keep privileged setup capability retrieval separate from observation. Move duplicated admission-format parsing behind its owning shared module interface without exposing secrets in status, logs, URLs, or frontend persistence.
- [x] Use typed shared responses rather than converting them to untyped JSON solely to inspect readiness fields. Browser/remote observation continues through supported interfaces without making Tauri a prerequisite or proxy for all remote operations.
- [x] Route transient failures to retry, prerequisite failures to actionable instructions, and damaged retained identity to repair; reset is not the default recovery action.
- [x] Replace existing consumers and delete superseded observation commands and duplicate state logic in this slice; retain commands still required for distinct protected operations. Do not add compatibility wrappers.
- [x] Verify externally visible setup and authentication routing using existing rendered-page and HTTP/native-adapter test seams, including malformed evidence, target changes, and local failure with remote selection.
- [x] Update relevant interface and native handoff documentation and run focused tests/checks; record outcomes and qualification limits.

## Answer

Implemented one typed `OnboardingObservation` for Installation setup, setup entry, Device preference routing, direct sign-in and authenticated navigation. It projects starting / needs setup / working / ready / blocked from current evidence, carries safe target/action information and verified public login metadata, and discards stale observations when any selected target changes. Browser/remote observation stays direct HTTPS and never depends on bundled failure or native capabilities.

Bundled `get_onboarding_host` returns typed shared status and categorical platform failure, not raw startup errors or secrets. Removed `get_setup_stage`, `get_desktop_client_id`, `get_native_host_error`, `getSetupStage`, `getWebSetupHosts` and `fetchSetupStage`, with no compatibility wrappers. Privileged capability retrieval remains separate and uses the shared admission module's existing-only protected reader. Existing Create/Join and verified Device preference behavior are retained. Runtime retry, prerequisite instructions and exact repair/recovery are distinguished; reset is explicit maintenance, never the default recovery action. Source approval UI was not edited.

Updated stable worker/input/action documentation, API/operations guidance and the native handoff. [Evidence and qualification limits](../evidence/01-onboarding-observation.md): 181 frontend state/route tests, eight mounted interactions, 40 API tests plus doc test, 17 native tests plus explicitly launched provider child, two shared-host tests, app build/typecheck and changed-crate strict Clippy pass. TDD red/green cases are recorded there. Full feature validation remains blocked by the Swagger UI archive build failure; CRAP timed out during compilation. The standalone HTTP script retains the same 15 baseline failures, independently reproduced against the starting revision.

Confirmed integration ancestry at `dfeaf7dbe`; merged `setup-simplification` into this branch (`Already up to date`).
