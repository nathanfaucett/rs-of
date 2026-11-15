# 03: Migrate the service crates to `ofdb` names

**What to build:** Rename the SQL data-layer modules and features in the Device, IdP, and Management service crates from `replica` to `ofdb` (repo implementations, schema helpers, validators). After this ticket, services expose e.g. `device_service::ofdb::...` and their `ofdb` feature is the canonical one; old paths still work via ticket 01 aliases.

**Blocked by:** 01 (Expand — `ofdb` aliases beside `replica`), 02 (Migrate `of-model` to `ofdb`)

**Status:** ready-for-agent

- [ ] Each service crate's SQL modules are reachable under `ofdb` paths
- [ ] Each service crate's `ofdb` feature replaces `replica` in its own manifests
- [ ] Service crates compile and their tests pass with both names available
- [ ] No replication/sync terminology was renamed
