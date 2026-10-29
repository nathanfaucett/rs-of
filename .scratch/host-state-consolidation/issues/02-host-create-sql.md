# 02: Move host configuration and Create setup into SQL

**What to build:** A fresh Unified Host reaches awaiting-choice setup and completes Create using one private host-local `ofdb-sql` database for configuration and workflow state. Its temporary setup-code file contains credential material only. Exact retries, authorized recovery, cancellation, completion, restart, and reset retain their current guarantees without Create's overlapping file journals.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] Reuse the existing SQL engine in a dedicated host-local database opened before domain provisioning. Do not add SQLite, expose this database through domain SQL/replication, or merge owner database lifecycles.
- [ ] Keep a minimal versioned representation of host configuration and Create operation binding, stable allocated identities, credential/provider references, attempt evidence, acknowledgements, completion, and retirement. Preserve owner-specific domain authority and protected inventories.
- [ ] Fresh host creation, awaiting-choice restart, and exclusive process ownership work end to end. Missing/corrupt retained databases, unsupported schema, and unsupported legacy layouts refuse intact before any implicit database creation, credential generation, or repair.
- [ ] Separate setup credential handling from workflow history for Unified Create. Preserve existing header authorization, protected local code retrieval, and authorized nonsecret intent recovery. Do not add code expiry, rotation, single-use semantics, or a new sign-in flow.
- [ ] Bind the exact canonical Create operation and required initial attempt evidence in a host transaction before privileged mutation or acceptance. Reject conflicting requests and retain identities and choices across exact retries and interruptions.
- [ ] Preserve cancellation/finalization serialization, current owner readiness checks, and completed Create retry correlation followed by normal Initial Administrator authentication. Cancellation must not claim rollback.
- [ ] Commit Create completion and admission retirement together after required owner publication and verification. Delete the temporary code afterward; a leftover code after cleanup failure or restart never authorizes setup. Missing live code refuses without regeneration, while retired reopen requires no code.
- [ ] Keep external effects outside host write transactions: commit attempted evidence, perform owner/provider work, then commit acknowledgement. Do not claim atomicity across owner stores or providers.
- [ ] Update Create reset and native/shared reference recovery to use authoritative SQL metadata. Retain exact references through interrupted cleanup and do not delete shared owner data.
- [ ] Delete replaced host configuration and Create journal parsing/publication code in this slice. Leave Join-specific persistence for ticket 03, with existing Join behavior still working; do not add legacy-layout adapters or parallel authoritative copies of converted facts. Ticket 03 will reuse the new host store and admission handling.
- [ ] Preserve independent-owner admission behavior when shared code changes. Do not move independent-owner journals or domain databases into Unified storage.
- [ ] Extend existing host lifecycle tests for fresh Create, awaiting-choice reopen, exact/conflicting/concurrent requests, authorized recovery, cancellation, completion/retirement atomicity, leftover-code denial, completed reopen, missing/corrupt state, creation interruption, and reset. Use fault seams only where required to prove an observable safety guarantee.
- [ ] This ticket does not depend on provider-backed endpoint persistence. Use whichever endpoint implementation is authoritative when it lands, without a new compatibility abstraction.
- [ ] Update affected setup, security, operations, and native documentation. Run focused lifecycle/admission regressions and necessary affected compile/format/lint checks; record results and limits. No full cargo-hack feature-powerset run or CRAP testing is required.
