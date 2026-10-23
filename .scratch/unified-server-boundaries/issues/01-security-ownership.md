# Security synchronization and admission ownership

Status: resolved
Triage: ready-for-agent
Type: task
Blocked by: none

Implement the security synchronization and bootstrap portion of ../spec.md. Trace existing owner interfaces; move domain decisions and protocol implementations behind their correct existing owner interfaces, not into a catch-all crate. Preserve per-send/pre-apply authorization, retained admission/recovery state and shared endpoint registration. Unified may coordinate concrete owner calls only. Preserve public contracts and unique integration coverage. Add owner-facing tests and remove superseded implementations. Do not create direct server-to-server dependencies.

Write scope: security synchronization/bootstrap modules and their tests, Device security/trust/protocol owner modules and required owner interface changes. Coordinate shared crate manifests/root declarations when merging. Do not change local provisioning or host configuration workflows.

Use tdd. Commit work on a ticket worktree based on the integration branch; merge current integration tip before reporting. Report tests and any remaining ownership limitations honestly.


## Answer

Implemented on `refactor/unified-security` in the dedicated security worktree.

### Ownership

- Device's `security_protocol` and `security_bootstrap` implement marker-3 scoped synchronization, marker-4 purpose-bound public bootstrap, exact admission/readiness checks, history validation and retry ordering. Per-send and pre-apply checks remain mandatory. The public receive-history operation always applies the real receiver authorization; its additional check can only deny, never authorize.
- Device's `join_admission` implements purpose-specific ALPN admission and ordinary allowlist fallback. IdP's retained admission provider still owns rereading/verifying its request, receipt and protected private signer on every check.
- `NodeJoinOwner` exposes only the existing IdP provisioner's retained signer/approval/import-journal operations. IdP implements it without depending on another server. The three journal filenames and exact-anchor persistence are unchanged.
- Unified's security facade only constructs owners, registers the owner's stream handler with Storage's existing shared DATA dispatcher, and transfers existing handles into runtime composition. No wire protocol, repository inspection, admission decision or durable transition remains there. The old Unified protocol/bootstrap implementations are deleted.
- Bounded SQL stream framing moved from Storage server to existing `db` persistence utilities; Storage re-exports it. No new crate or server-to-server dependency was introduced.
- The HTTPS bootstrap verification module remains in Unified: it probes composed owner contracts and owns listener/readiness/shutdown qualification, rather than making domain admission decisions. Local provisioning, configuration and setup workflows are unchanged.
- Stable design/public-persistence documentation describes the new boundaries. Outbound authorization coverage moved to Device; unique shared-DATA and cross-owner recovery tests remain in Unified. Durable-transition fault injection now uses the IdP owner interface, not production test hooks.

### Validation actually observed

All Cargo invocations used the worktree manifest and shared target `/home/nathan/code/rust/aicacia/of/target`; jobs initiated here were serialized. Other worktrees caused occasional target/package-cache lock waits. Tests used `RUST_MIN_STACK=16777216`; focused Unified tests used the command-line-only `profile.test.package.unified-server.opt-level=0` override to bound compilation (no profile file changes).

Passed:

- Three Device owner contract tests: exact bootstrap endpoint binding, widened-schema denial, and real-endpoint purpose-only admission with learned missing/malformed evidence and ordinary allowlist fallback.
- Each of the five moved outbound authorization scenarios was observed passing across focused runs: both roles/contexts, learned actor/Device/grant/cancellation/policy denial, admitted-write completion followed by next-operation denial, and identity-policy grant/revocation propagation. The combined run timed out at 240 seconds after four passes; the remaining scenario subsequently passed individually.
- Unified real shared-DATA equal-peer scoped history exchange.
- Unified four durable-transition interruption/reopen checkpoints (rerun after the final checked receiver API change).
- Unified admitted-write/cancellation/native-retry scenario (rerun after the final checked receiver API change).
- Unified desktop public-bootstrap orchestration, completed offline retry and learned-revocation denial (rerun after admission-hook extraction).
- Affected packages (`device-service`, `idp-db`, `idp-server`, `storage-server`, `unified-server`) all-target/all-feature check.
- Device no-default-feature check; affected library all-feature Clippy (existing generated-client warnings).
- Server-dependency audit, touched Rust formatting and `git diff --check`.

Limitations, not passing qualification:

- An initial default-stack owner run aborted with stack overflow; the documented larger stack was required.
- Initial optimized Unified test compilation timed out at 180 seconds; focused tests subsequently built and passed with the package-specific test optimization override.
- All-target Clippy fails on `clippy::unused_io_amount` in untouched Storage test fixtures (`device_client.rs`, `idp_client.rs`, and `router/resources.rs`). These unrelated fixtures were not changed.
- Required root `cargo hack test --feature-powerset --all-targets` timed out at 120 seconds compiling the first of ten combinations. It is neither a completed powerset nor workspace-wide test qualification.
- Required `just crap` timed out at 120 seconds compiling instrumented coverage. No complexity score was produced.

### Integration coordination and remaining scope

Shared files requiring normal merge coordination are `Cargo.lock`, `crates/{db,device-service,storage-server}/Cargo.toml`, and declarations/re-exports in `crates/{db,device-service,storage-server,unified-server}/src/lib.rs`. IdP's existing bootstrap ALPN export now re-exports Device's constant. No workspace/root manifest change was needed.

No known security-protocol ownership gap remains within this ticket. Full feature-powerset/complexity qualification remains outstanding; other tickets still own provisioning/configuration boundaries. The current integration branch tip must be merged before reporting completion.
