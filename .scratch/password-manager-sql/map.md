# Password-manager SQL map

## Notes

- [Spec](spec.md) defines two independently verifiable slices.
- [01: explicit authenticated SQL](issues/01-authenticated-sql-access.md) is resolved;
  its Answer records the precise ticket-02 API/identity handoff and actual checks.
- [02: application cutover](issues/02-password-manager-sql.md) is resolved; its Answer records delivered behavior, validation and deployment qualification limits.

- [03: all review fixes](issues/03-review-fixes.md) is resolved; records all five fixes, the required neighboring owner commit and actual validation/limits.

## Decisions-so-far

- [01](issues/01-authenticated-sql-access.md): generic explicit keyed ensure derives
  UUID v5 identity from the verified User/canonical-Application/key tuple; durable
  initializing/ready lifecycle, permanent Tombstones and fail-closed unavailable
  content. Independent Nodes share identity, not global write serialization.
- [01](issues/01-authenticated-sql-access.md): one bounded Authorization-header SQL
  HTTP batch uses parsed owner operations and a public owner transaction. Generated
  contracts, explicit Storage client methods and generation-guarded SDK access are
  delivered without app defaults/schema/Device selection. Filesystem APIs remain.
- [01](issues/01-authenticated-sql-access.md): native SQL HTTP (12), filesystem HTTP
  (7), server library (95), client (34), SDK (52) and service/client feature checks
  pass; strict changed-library lint/build/format/generation checks pass. Broader
  validation failures/timeouts are recorded in the ticket, not hidden as passes.

- [02](issues/02-password-manager-sql.md): the app explicitly ensures permanent key `password-manager:default`, initializes schema/version transactionally, checks existing version state and opens an app-owned SQL collection. SDK/collection provisioning policy remains absent; no selector, filesystem fallback/import, or replacement key exists.
- [02](issues/02-password-manager-sql.md): full CRUD/Refresh/metadata/history, official TanStack binding, reconciliation, uncertain-write handling and synchronous session cleanup are delivered. App **65**, SDK **52**, client **34**, generic filesystem collection **59**, native SQL HTTP **13** and config **2** tests pass; build/types/focused lint/format/autofixer pass. Powerset/CRAP timeouts, existing strict integration-test lint debt and unavailable deployed browser qualification are documented, not represented as passing.

- [03](issues/03-review-fixes.md): acknowledged resources use public existing-only opens; RETURNING requires both actions; interrupted initialization is fenced/cleaned across restart. App mutations and snapshots share one atomic batch. HTTP exposes a restricted generic CRUD subset with owner pre-allocation native admission and decoded-row accounting, not unrestricted SQL. Required `ofdb` commit: `69813e339ef5d4d5bba3dd896488f1d2b0e379a9` on its separate `implement/password-manager-sql-review` branch.
- [03](issues/03-review-fixes.md): native SQL HTTP **18**, filesystem HTTP **7**, Storage service **25**, Storage server **95**, app **67** and config **2** pass; focused owner persistence/budget/native/2 MiB sync tests and strict changed-library lint pass. No full powerset/CRAP/deployed-qualification claim; see ticket 03 for reproduced baseline owner failures.

- [03 P1 follow-up](issues/03-review-fixes.md#p1-follow-up-selected-open-acknowledgement-bypass): after merging integration `a59f0c3c`, selected/protocol SQL opens are existing-only unless explicit durable fresh-copy intent exists. Intent survives fresh bootstrap interruption and is consumed before any SQL owner is cached; neither missing files nor reselected generation numbers imply freshness. Restart regression covers missing/zero/junk content with selected opening first, for original and reselected copies. Service powerset **27** and server library **95** pass; neighboring repositories are unchanged.

## Fog

- No-feature Storage server export defect and existing all-target lint debt remain
  baseline limits, not additions to the application-cutover scope.
- Full server powerset and workspace coverage/CRAP did not complete within bounds.
- Restricted HTTP SQL plus native/decoded read budgets bound scan retention and
  reject query amplification; they are not a process-memory/decompression/CPU or
  concurrent-request sandbox. A codec decodes one row before logical-size admission.
- Owner broad-suite catalog timing and outgoing-candidate callback failures were
  also reproduced on the untouched owner base; full powerset/CRAP remain incomplete.
- No deployed browser/OS/provider or mesh-convergence qualification is inferred.
  Crash interruption coverage does not enumerate every power-loss cutpoint.
