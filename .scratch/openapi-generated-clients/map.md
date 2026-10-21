# OpenAPI generated clients map

## Notes

Implementation branches/worktrees are user-authorized. Ticket 01 work is isolated from the parent checkout.

## Decisions-so-far

- [01: Shared specs and generated clients](issues/01-shared-specs-and-generated-clients.md) — resolved. Ten state-free deterministic canonical documents feed both pinned generators; staged validation protects both SDK families. Actual owner/Setup/unified Node-join listeners and bounded Rust transport are qualified. See ticket Answer for commands and validation caveats, and `docs/openapi-inventory.md` for the migration queue.

- [02: Generated HTTP consumers](issues/02-replace-handwritten-http-calls.md) — resolved. All inventoried project-owned production HTTP operations use generated implementations/builders; local owners, transport limits, renewal and domain checks remain. Offline qualification, actual owner/Join/Storage upgrade checks, 225 browser/package tests, 53 focused Rust tests, root feature powerset and library Clippy pass. Broader-suite, coverage and existing fixture/tooling limits are explicit in the ticket.

## Fog

- Ticket 03 owns CI enforcement and final migration reconciliation.
- Full workspace CRAP/coverage did not finish within the bounded runs; existing all-target test clippy lints remain.
