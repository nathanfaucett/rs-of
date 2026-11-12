# Ticket 02: Database Abstraction Trait

**Status: ready-for-agent**

**Blocked by:** 01

## What it delivers

A `Database` trait in `of-model` (or new `of-db` crate) that abstracts storage behind a common interface. Owner services (`idp-server`, `device-server`, `management-server`, `storage-server`) are refactored to accept `Arc<dyn Database>` instead of concrete database types.

**Breaking changes expected** — owner service signatures change to accept `Database` trait. No dual-support.

## Scope

- Define `Database` trait in `of-model::db`:
  ```rust
  pub trait Database: Send + Sync {
      type Txn: Transaction;
      fn begin(&self) -> Result<Self::Txn, DbError>;
      // Owner-specific methods as associated types or generic params
  }
  ```
- Define `Transaction` trait with CRUD operations
- Define `DbError` unified error type
- Refactor `idp-server::provisioning::OwnerProvisioner` to use `Database`
- Refactor `device-server::provisioning` to use `Database`
- Refactor `storage-server::provisioning` to use `Database`
- Refactor `management-server::provisioning` to use `Database`
- **Delete** old concrete database imports from owner services
- Keep existing redb/PostgreSQL implementations as concrete types implementing `Database` (for independent deployments)

## Acceptance Criteria

- All four owner servers compile with `Database` trait
- No owner service imports concrete database types directly
- Existing independent deployment tests still pass (they provide NetworkDatabase)
- `Database` trait is object-safe (or use `impl Database` pattern)
- Minimal trait surface — only what owners actually need
