# Password-manager SQL map

## Notes

- [Spec](spec.md) defines two independently verifiable slices.
- [01: explicit authenticated SQL](issues/01-authenticated-sql-access.md) is resolved;
  its Answer records the precise ticket-02 API/identity handoff and actual checks.
- [02: application cutover](issues/02-password-manager-sql.md) is resolved; its Answer records delivered behavior, validation and deployment qualification limits.

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

## Fog

- No-feature Storage server export defect and existing all-target lint debt remain
  baseline limits, not additions to the application-cutover scope.
- Full server powerset and workspace coverage/CRAP did not complete within bounds.
- SQL request/result limits do not isolate hostile query CPU/scans; current public
  owner execution materializes results before the response bound is checked.
- No deployed browser/OS/provider or mesh-convergence qualification is inferred.
  Crash interruption coverage does not enumerate every power-loss cutpoint.
