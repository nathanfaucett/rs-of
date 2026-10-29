# 03: Move Join onto the same host store

**What to build:** Unified Join uses the host-local SQL store and admission/workflow separation established for Create. Operators can join the exact Installation, recover and retry an interrupted operation, restart the completed Node offline, and explicitly reset it without separate Join file journals.

**Blocked by:** 02 — Move host configuration and Create setup into SQL.

**Status:** ready-for-agent

- [ ] Reuse the existing host store, transaction handling, temporary credential handling, and durable retirement model from ticket 02. Do not introduce a second store, repository framework, or parallel operation history.
- [ ] Persist the exact canonical Join operation, trusted peer, authenticated issuer, Node choice, stable allocated identities, credential references, attempts, and acknowledgements in host metadata. Preserve authenticated discovery and source approval semantics.
- [ ] Bind durable operation evidence before acceptance or privileged provisioning. Exact retries use retained choices and identities; conflicting Create/Join requests or peer/issuer substitution refuse.
- [ ] Authorized nonsecret recovery observes without resuming work. Preserve cancellation, interruption, approval/rejection handling, and the distinction between cancellation and rollback.
- [ ] Commit Join completion and admission retirement together after required verification/publication. Remove the temporary code afterward; cleanup failure cannot restore authority. Completed-operation checks read host metadata rather than admission-owned historical payloads.
- [ ] Completed restart requires only existing local state, verifies the retained endpoint binding, and works without contacting the original Join peer. Current readiness and normal authorization remain separate from setup completion.
- [ ] Preserve attempted-before-external-effect and acknowledged-after-effect ordering. Provider, owner, and network failures never silently replace identities, repair missing evidence, or claim distributed rollback.
- [ ] Update Join reset and native/shared recovery to use exact retained host references, preserve continuation evidence until secret cleanup succeeds, and leave shared or remote owner data untouched.
- [ ] Remove superseded Join journals, duplicated intent storage, parsing/publication code, and remaining obsolete Unified admission workflow-history handling. Retain genuine TLS/configuration artifacts and owner-specific inventories that are outside the consolidation.
- [ ] Unsupported legacy layouts remain intact with an actionable refusal; do not add migration, dual reads/writes, or serving wrappers.
- [ ] Extend existing Join lifecycle and native/shared tests for approval, interruption, exact/conflicting retries, recovery, cancellation, atomic retirement, missing/corrupt metadata, offline completed reopen, and interrupted exact reset. Run affected Create and independent-owner admission regressions for shared changes.
- [ ] If ticket 01 has landed, include focused combined lifecycle coverage for SQL-backed metadata and provider-backed endpoint identity, especially stable reopen and cleanup failure. Ticket 01 does not block this slice; if it lands later, its focused validation must exercise the then-current SQL-backed Create/Join paths. No separate integration ticket is needed.
- [ ] Update existing operations, setup, security, and native documentation and remove obsolete guidance in this ticket. Record focused checks and remaining qualification limits; do not access real providers/OS trust or run a full cargo-hack feature-powerset or CRAP testing campaign.
