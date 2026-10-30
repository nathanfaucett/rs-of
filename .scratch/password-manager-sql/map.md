# Password-manager SQL map

## Notes

- [Spec](spec.md) defines two independently verifiable slices.
- [01: explicit authenticated SQL](issues/01-authenticated-sql-access.md) is resolved;
  its Answer records the precise ticket-02 API/identity handoff and actual checks.
- [02: application cutover](issues/02-password-manager-sql.md) is now unblocked.
  Ticket 01 does not change password-manager defaults/UI/persistence or import files.

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

## Fog

- No-feature Storage server export defect and existing all-target lint debt remain
  baseline limits, not additions to the application-cutover scope.
- Full server powerset and workspace coverage/CRAP did not complete within bounds.
- SQL request/result limits do not isolate hostile query CPU/scans; current public
  owner execution materializes results before the response bound is checked.
- No deployed browser/OS/provider or mesh-convergence qualification is inferred.
  Crash interruption coverage does not enumerate every power-loss cutpoint.
