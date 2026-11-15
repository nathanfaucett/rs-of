# 08: Contract — delete the old `replica` names

**What to build:** Remove the ticket 01 aliases: the old `replica` module names and `replica` cargo features on the SQL layer are deleted now that no caller references them. Replication/sync `replica` terminology (contract replication, replica signers) remains as-is.

**Blocked by:** 02 (Migrate `of-model` to `ofdb`), 03 (Migrate the service crates to `ofdb`), 04 (Migrate servers and manifests to `ofdb`)

**Status:** ready-for-agent

- [ ] No SQL-layer `replica` module or feature name remains anywhere in the workspace
- [ ] Workspace compiles and tests pass
- [ ] `replication` contract and replica-signer routes are still present and unchanged
