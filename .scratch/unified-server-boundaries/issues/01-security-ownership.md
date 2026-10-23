# Security synchronization and admission ownership

Status: open
Triage: ready-for-agent
Type: task
Blocked by: none

Implement the security synchronization and bootstrap portion of ../spec.md. Trace existing owner interfaces; move domain decisions and protocol implementations behind their correct existing owner interfaces, not into a catch-all crate. Preserve per-send/pre-apply authorization, retained admission/recovery state and shared endpoint registration. Unified may coordinate concrete owner calls only. Preserve public contracts and unique integration coverage. Add owner-facing tests and remove superseded implementations. Do not create direct server-to-server dependencies.

Write scope: security synchronization/bootstrap modules and their tests, Device security/trust/protocol owner modules and required owner interface changes. Coordinate shared crate manifests/root declarations when merging. Do not change local provisioning or host configuration workflows.

Use tdd. Commit work on a ticket worktree based on the integration branch; merge current integration tip before reporting. Report tests and any remaining ownership limitations honestly.
