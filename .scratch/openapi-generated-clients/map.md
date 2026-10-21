# OpenAPI generated clients map

## Notes

Implementation branches/worktrees are user-authorized. Ticket 01 work is isolated from the parent checkout.

## Decisions-so-far

- [01: Shared specs and generated clients](issues/01-shared-specs-and-generated-clients.md) — resolved. Ten state-free deterministic canonical documents feed both pinned generators; staged validation protects both SDK families. Actual owner/Setup/unified Node-join listeners and bounded Rust transport are qualified. See ticket Answer for commands and validation caveats, and `docs/openapi-inventory.md` for the migration queue.

## Fog

- Ticket 02 owns production-consumer migration with existing token/domain/local-owner behavior preserved.
- Ticket 03 owns CI enforcement and final migration reconciliation.
- Full workspace CRAP/coverage did not finish within the bounded runs; existing all-target test clippy lints remain.
