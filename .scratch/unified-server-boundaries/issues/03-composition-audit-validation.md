# Composition audit, documentation and final validation

Status: open
Triage: ready-for-agent
Type: task
Blocked by: 01, 02

Implement remaining host configuration/setup/endpoint/runtime boundary audit from ../spec.md. Preserve necessary composition and process lifecycle; remove owner-policy duplicates through owner interfaces. Review tests by assertions, not filenames. Record retained responsibilities and correct contradictory stable design documentation. Merge security and provisioning work; review complete diff for boundary and security regressions. Run focused tests then all-target/all-feature checks, formatting, Clippy, dependency checks, feature-powerset testing and just crap serially. Characterize failures and timeouts accurately. Resolve tickets only when implemented, explicitly record any unmet acceptance criteria.

No new features, file-count targets, catch-all crate, generic host framework or blanket test relocation.
