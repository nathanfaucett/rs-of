# 02: Migrate `of-model` to `ofdb` names

**What to build:** Switch the domain crate's SQL-facing surface to the `ofdb` name: the module holding the migration constant and the crate feature. After this ticket, `of_model::ofdb::MIGRATION` is the canonical path to the single init migration, and the old `of_model::replica` path still works via the alias from ticket 01.

**Blocked by:** 01 (Expand — `ofdb` aliases beside `replica`)

**Status:** ready-for-agent

- [ ] `of_model::ofdb::MIGRATION` resolves to the single init migration SQL
- [ ] The `ofdb` feature on `of-model` enables the same things the `replica` feature did
- [ ] All in-crate call sites use the `ofdb` name
- [ ] Crate still compiles with the alias in place
