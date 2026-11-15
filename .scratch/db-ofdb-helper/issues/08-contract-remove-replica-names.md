**Status:** resolved

- [x] No SQL-layer `replica` module or feature name remains anywhere in the workspace
- [x] Workspace compiles and tests pass
- [x] `replication` contract and replica-signer routes are still present and unchanged

## Answer

Removed all SQL-layer `replica` feature aliases and module aliases:

- of-model: deleted `replica = ["ofdb"]` feature; removed `pub use ofdb as replica` from lib.rs and idp/mod.rs
- db: deleted `replica = ["ofdb"]` feature
- device-service: deleted `replica = ["ofdb"]` feature; removed `pub use ofdb as replica` from lib.rs; fixed dangling comma in Cargo.toml
- idp-service: deleted `replica = ["ofdb"]` feature; removed `pub use ofdb as replica` from lib.rs
- management-service: deleted `replica = ["ofdb"]` feature; removed `pub use ofdb as replica` from lib.rs

Replication/sync terminology (`contract::replication`, replica-signer routes, etc.) untouched.

Workspace lib check passes.
