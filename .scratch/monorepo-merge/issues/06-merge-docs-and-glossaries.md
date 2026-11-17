# Ticket 06: Merge documentation and glossaries into single-context root

Status: ready-for-agent

Blocked by: (none — can run in parallel with crate copying)

## What it delivers

All domain terms defined in one `GLOSSARY.md` at the repo root; `GLOSSARY-MAP.md`, `README.md`, and package READMEs reference in-repo paths (`services/…`, `db/…`, `fs/…`, `net/…`) only.

## Scope

1. **`GLOSSARY.md`**: merge `../ofnet/GLOSSARY.md` (Endpoint, Peer, Mesh, Broadcast, Direct Message, Byte Stream), `../ofdb/GLOSSARY.md` (SQL, KV, sync, staging, CRDT, etc.), `../offs/GLOSSARY.md` (file-system persistence) into the existing root glossary; drop the `../ofnet/GLOSSARY.md` link
2. **`GLOSSARY-MAP.md`**: update context links `crates/<ctx>-service/…` → `services/<ctx>-service/…`; drop the `../ofnet/GLOSSARY.md` link; update ofdb/offs context rows to point at `db/` and `fs/`
3. **`README.md`**: replace references to `ofnet`/`ofdb`/`offs` as separate repos with in-repo layout; update the networking composition paragraph
4. **Package READMEs/comments**:
   - `packages/sql/README.md`: `../../crates/sql/README.md` → `../../db/sql/README.md` (or remove if that file doesn't exist)
   - `packages/example/CODING_STANDARDS.md`: `crates/…` → `services/…` / `db/…`
   - `packages/storage-sync-client/README.md` and `scripts/generate-fixtures.py`: `ofdb/crates/…` → `db/…`, `crates/…` → `services/…`
   - `packages/storage-client/README.md`: `crates/storage-service/…` → `services/storage-service/…`
   - `packages/sdk/README.md`: `crates/storage-service/…` → `services/storage-service/…`
   - `packages/idp-client/src/models/ErrorResponse.ts`: `crates/model/…` → `services/model/…`

## Acceptance

- `grep -rn '\.\./ofnet\|ofdb/crates\|offs/crates\|crates/' of/ --include='*.md' --include='*.ts' --include='*.py' | grep -v node_modules | grep -v 'test-results\|artifacts\|pkg'` returns nothing (except intentional historical comments)
- `GLOSSARY.md` contains Endpoint, Peer, Mesh, Broadcast, Direct Message, Byte Stream definitions
- All glossary links resolve to files in `of/`
