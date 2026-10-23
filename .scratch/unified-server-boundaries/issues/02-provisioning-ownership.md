# Provisioning and Join owner interfaces

Status: claimed
Triage: ready-for-agent
Type: task
Blocked by: none

Implement provisioning/Join ownership portion of ../spec.md. Audit local provisioning and local Join, durable journal transitions and related recovery checks. Remove domain-state interpretation and direct owner repository access from Unified by using concrete existing owner interfaces; add minimal owner operations for existing behavior where missing. Keep legitimate host ordering and nonsecret process references. Do not relocate an entire cross-owner implementation into a server or introduce a generic callback framework. Preserve journal formats, exact retries, credentials, signer identity, fail-closed recovery and offline reopen. Add owner-facing tests; retain unique composition tests.

Write scope: local_provisioning and local_join modules/tests, IdP provisioning owner modules and Management/Storage provisioning interfaces as required. Do not edit security protocol modules, Device security policy, setup_host/setup_runtime or host config modules. Shared manifests/root declarations will be reconciled at merge.

Use tdd. Commit work on a ticket worktree based on integration branch; merge integration tip before reporting. Report validation and unresolved limitations honestly.
