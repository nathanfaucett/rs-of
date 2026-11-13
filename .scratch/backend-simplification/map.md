# Backend Simplification Map

## Notes

Unified spec combining model extraction + unified-server replacement for E2E testing.
Aggressive breaking changes: delete old code, no compat shims, replace unified-server binary.

## Decisions-so-far

- Single model crate: `of-model` (no_std + serde)
- Database trait in `of-model`, three impls: Embedded/Network/Memory
- Embedded DB: `embedded-db` crate using the existing SQL layer over redb, single file `unified.redb`. [Ticket 02](issues/02-embedded-database.md) adds migrated Embedded/Memory/Network adapters and real persistence/shared-schema/network tests. Runtime requires std; Network uses ofdb SQL, not PostgreSQL. Ticket remains claimed pending strict acceptance and ticket 01.
- Replace unified-server entirely (not add simple variant)
- CLI: `cargo run -p unified-server -- local --data-dir --port --origin [--no-tls]`
- Auto-seed: admin user + OIDC clients for frontends
- TLS: auto-generate self-signed or `--no-tls` for HTTP
- E2E tests: Playwright globalSetup spawns real backend, hits HTTP endpoints
- Setup lifecycle is retained for independent deployments. The unchanged setup crate remains a required transitive dependency of local composition through owner servers and server-common; graph exclusion needs a separate owner/common split. [Ticket 06](issues/06-cleanup-legacy-setup.md) removes only Management's unused direct edge and records [cleanup validation](cleanup-validation.md).
- Delete: api, idp-model, storage-model, management-model, setup_host, provisioning, bootstrap, independent__, node_join_, of/data/unified/

## Fog

- Ticket 06 remains claimed: legacy paths are already absent, but required transitive setup edges remain. The setup library builds; unchanged setup test targets fail. Workspace checking fails on sibling snapshot serde support; feature/test matrices and CRAP checks did not complete. Independent owner HTTPS tests retain deleted API fixture paths. No further tracked obsolete data or crates can safely be deleted in the cleanup scope.

- Unified SQL schema is initialized through `of_model::replica::up`; Embedded/Memory/Network adapters use that same schema. Strict alloc-only runtime scope conflicts with redb and the reused SQL engine's std requirements.
- Ticket 01 still has domain/wire contract duplication (OAuth user requests, setup/join payloads, SDK sync status). TLS listeners, host controls, and authenticated extractors are runtime types, not duplicate domain entities. See its assessment before further consolidation.
- TLS cert generation details (rcgen vs existing TLS code)
- Port allocation strategy for parallel test runs (port 0 for dynamic?)
- Seed data format (JSON file vs hardcoded vs admin API)
- CI integration: GitHub Actions needs cargo + frontend deps

## Tickets

| #   | Title                                            | Type | Status          | Blocked by |
| --- | ------------------------------------------------ | ---- | --------------- | ---------- |
| 01  | Extract Single Model Crate (`of-model`)          | task | claimed         | —          |
| 02  | Embedded Database Implementation (`embedded-db`) | task | claimed         | 01         |
| 03  | Refactor Owner Services for Shared Database      | task | ready-for-agent | 01, 02     |
| 04  | Replace Unified Server Binary                    | task | ready-for-agent | 03         |
| 05  | E2E Test Integration (Playwright + Vitest)       | task | ready-for-agent | 04         |
| 06  | Cleanup Legacy Setup Code                        | task | claimed         | 04         |
