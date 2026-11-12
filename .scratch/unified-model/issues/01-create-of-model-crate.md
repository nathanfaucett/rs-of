# Ticket 01: Create `of-model` crate with consolidated types

**Status: ready-for-agent**

**Blocked by:** None

## What it delivers

A new crate `of/crates/of-model/` that re-exports all domain types from the four existing model sources. After this ticket, `cargo check -p of-model` passes and all types are importable from `of_model`.

## Scope

1. Create `of/crates/of-model/Cargo.toml` with dependencies: `db`, `idp-model`, `device-service`, `storage-model`, `management-service`, `serde`, `uuid`
2. Create module structure:
   - `src/lib.rs` — re-exports all public types
   - `src/id.rs` — `Id` (UUID v7)
   - `src/user.rs` — User, emails, phones, credentials, MFA
   - `src/application.rs` — Application, Client, OAuth2 types
   - `src/permission.rs` — Role, Permission, UserRole, ApplicationUserRole
   - `src/device.rs` — Device, Enrollment, SelectionPolicy, ResourceSelection
   - `src/storage.rs` — Namespace, ResourceCatalog, ResourceIdentity, ResourceKind, Session, Entry, Request, Response
   - `src/replication.rs` — Sync, Admission, CopyRetirement, SyncSelection
   - `src/replica.rs` — placeholder `up()` function
3. Move type definitions from existing crates into `of-model` (or re-export via `pub use`). Start with re-exports to avoid breaking changes; inline definitions in later tickets.
4. Add `of-model` to workspace `Cargo.toml` members.

## Acceptance criteria

- `cargo check -p of-model` succeeds
- `cargo doc -p of-model` succeeds
- All four server crates can add `of-model` as dependency and import types without version conflicts
