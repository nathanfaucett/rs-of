# `ofdb` facade boundary

**Status:** accepted

`of` consumes `ofdb` only through the `ofdb-sql` and `ofdb-kv` facades. `Database` owns lifecycle and synchronization; `Client` owns queries and transactions. Storage engines, Redb shared hosts, Automerge details, and internal adapters stay behind that boundary so application services do not depend on persistence implementation details.

## Consequences

Services must not import internal engine crates. One process does not imply one physical database file. Existing shared-host facade or internal-crate integration is migration work, not a pattern to extend.
