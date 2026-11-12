# Ticket 02: Write unified migration `0001_init.up.sql`

**Status: ready-for-agent**

**Blocked by:** 01

## What it delivers

A single SQL migration file that creates all tables for IdP, Device, Storage, and Management in dependency order. Running `of_model::replica::up(&db)` on a fresh database creates a complete schema.

## Scope

1. Create `of-model/replica_migrations/0001_init.up.sql`
2. Concatenate existing migrations in correct order:
   - IdP tables first (users, credentials, applications, clients, keys, OAuth2, roles, permissions, replica members/signers)
   - Device tables (devices, enrollments, selection_policies, resource_selections, copy_cleanup)
   - Storage tables (namespaces, resources, databases, filesystems, sessions)
   - Management tables (sync_selections) — already covered by IdP migration
3. Handle foreign key dependencies: users → credentials → clients → OAuth2 tables; applications → clients; devices → selection_policies
4. Implement `of_model::replica::up()` to execute the migration (same pattern as `idp_model::replica::up`)
5. Add validation function that checks all expected tables exist

## Acceptance criteria

- `cargo test -p of-model` runs migration against in-memory DB and validates all tables
- No foreign key violations on fresh DB
- All tables from four existing migrations present
