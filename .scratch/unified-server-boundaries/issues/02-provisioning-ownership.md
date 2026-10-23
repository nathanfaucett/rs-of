# Provisioning and Join owner interfaces

Status: resolved
Triage: ready-for-agent
Type: task
Blocked by: none

Implement provisioning/Join ownership portion of ../spec.md. Audit local provisioning and local Join, durable journal transitions and related recovery checks. Remove domain-state interpretation and direct owner repository access from Unified by using concrete existing owner interfaces; add minimal owner operations for existing behavior where missing. Keep legitimate host ordering and nonsecret process references. Do not relocate an entire cross-owner implementation into a server or introduce a generic callback framework. Preserve journal formats, exact retries, credentials, signer identity, fail-closed recovery and offline reopen. Add owner-facing tests; retain unique composition tests.

Write scope: local_provisioning and local_join modules/tests, IdP provisioning owner modules and Management/Storage provisioning interfaces as required. Do not edit security protocol modules, Device security policy, setup_host/setup_runtime or host config modules. Shared manifests/root declarations will be reconciled at merge.

Use tdd. Commit work on a ticket worktree based on integration branch; merge integration tip before reporting. Report validation and unresolved limitations honestly.


## Answer

Implemented the scoped IdP/Management/Storage provisioning ownership extraction in `c22007dcd`.

- IdP owns public operator-login registration construction, the transparent retained password verifier, retained/configured signer binding validation, prepared/approved signer comparisons, and existing-only identity database preflight. Its existing protected secret inventory remains the authority for signer preparation and recovery; host attempt evidence never authorizes a replacement.
- Management owns `InitialAdministratorGrant`: stable role/permission references, inventory validation, and the explicit initial-administrator action mapping. Its flattened serialization preserves the existing `role` and `permissions` journal fields.
- Storage owns acknowledged runtime-root preflight, including symlink rejection and missing/non-directory denial without repair. Create and Join call this same owner operation.
- Unified retains concrete owner construction/reuse, cross-owner call ordering, process locks, exact configuration/root/identity references, cancellation/publication checks, and durable attempt/acknowledgement fences. These fences prevent automatic reinitialization after interrupted calls; owner operations still decide actual identity, signer, grant, and Storage validity. Completed reopen still verifies current owner evidence locally, not an old completion bit or a live peer.
- No entire cross-owner workflow moved into a server; no callbacks, new server dependency edges, protocol changes, or compatibility layer were introduced. Removed superseded host registration, verifier, signer-comparison, administrator-action mapping, and Storage-layout interpretation. Moved the Argon2 dependency from Unified to IdP. Updated the stable service-ownership documentation.
- Kept unique composition/recovery coverage. Added owner-facing contract tests and a flat retained-journal schema/readback/retry regression. Corrected the local recovery fixture to remove the Device root it actually initialized, not an absent historical Management root.

Risk assessment: security/recovery impact is high; implementation complexity is limited to concrete owner operations and delegation. The unchanged retained formats, secure-provider state machine, exact retry checks, construction ordering, and existing-only recovery fences were the priority.

### Validation

- Seven focused owner tests passed: retained password verifier, existing-only IdP preflight, operator-login registration/exact retry, host/owner signer evidence, configured binding substitution denial, administrator grant retry/offline reopen, and existing-only Storage preflight.
- Nine focused Unified tests passed: flat journal/readback, exact Create/offline reopen, exact interrupted continuation, missing signer denial, pre-enrollment readback interruption reuse, public exact Join intent, lost acknowledged Device/legacy Management phase, lost acknowledged Storage, and host-reference-driven Create/offline reopen.
- `cargo clippy -p idp-server -p management-server -p storage-server -p unified-server --no-default-features --lib --features storage-server/network --no-deps -- -D warnings` passed.
- Changed Rust files pass rustfmt; `git diff --check` and `scripts/check_server_dependencies.py` passed (no direct server-to-server edges).
- Default optimized Unified test compilation and a subsequent shared-target lock wait exceeded 120-second bounds. Focused tests completed with the same bound using `--config profile.test.package.unified-server.opt-level=0`, shared `of/target`, and `RUST_MIN_STACK=16777216`. No production profile was changed.
- Clippy without `--no-deps` stopped on 93 generated IdP-client lint errors; generated clients were not modified.
- The retained full-node test `local_provisioning_tests::full_nodes::two_completed_full_hosts_join_partition_reopen_and_learn_denial` failed before Join at unchanged `full_node_tests.rs:709`: its fixture opens `device/devices.redb` for Management permissions and receives `Invalid query: Table not found`. This was not baseline-qualified or repaired here; completed two-host Join is not claimed as passing.
- Workspace feature-powerset/all-target/all-feature qualification and `just crap` remain deferred to parent integration, per the instruction to avoid broad expensive validation in the ticket worktree.
- Merged the current `refactor/unified-server-boundaries` tip after the implementation commit (already up to date at `24f2179f9`).

### Remaining ownership gaps / handoff

- Local Create/Join still inspect the retained `devices.redb` file as a fail-closed construction fence. Device lacks a concrete existing-only provisioning preflight. Replacing this requires a Device-owner interface outside this ticket's permitted IdP/Management/Storage owner changes; no Device policy/protocol or security-bootstrap files were edited.
- Join still calls the existing Unified `SecurityProtocol` bootstrap entry point. Protocol ownership belongs to the separate security ticket, not a relocated provisioning workflow.
- `setup_host`, configuration/runtime audit, and final shared manifest/root-declaration reconciliation remain with the parent as requested. This resolves the scoped provisioning changes, not the entire multi-ticket spec or the outstanding full-node qualification gap.
