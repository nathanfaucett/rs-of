# 05 — Prove browser-only durability and offline use

Status: needs-triage
Work status: open
Type: task
Blocked by: 04

## Goal

Record real-browser evidence that the standalone app owns usable, durable data.
Do not treat mocked transport tests as acceptance.

## Scope

Test the built app and actual public WASM packages in the supported browser set.
Add the smallest offline asset mechanism needed for reload without the asset host.
Cache versioned application/WASM assets only. Keep vault data in its owner store.
Record browser versions, build revisions, limits, and test results without secrets.

## Acceptance

- First download, create, CRUD, lock, reload, browser restart, and unlock retain
  records with no native host, Installation, or sync backend.
- With network disabled and the asset host stopped, reload and CRUD still work.
- Quota, storage denial, private-mode limits, malformed content, missing known
  content, incompatible schema, interrupted commit, and asset upgrade fail safely.
- Inspect browser persistent storage/history: sensitive record data is ciphertext.
- Two tabs cannot silently overwrite credentials; focus/resume refreshes safely.
- Lock/vault switch/disposal suppress late loads and mutation notices.
- Encrypted export/restore works in a clean browser profile; backup and complete
  site-data-loss limits are clear to the user.
- Run app tests/build/check plus changed SDK/collection checks. For owner Rust
  changes, run browser acceptance, fmt/clippy, final feature-powerset tests, and
  `just crap`. Record blocked/failing commands without claiming a pass.
- Update app/package docs to describe only behavior this gate actually proves.

## Comments

Production-vault approval still requires separate security review. Native owner
restart tests and persistence permission do not guarantee browser backup or erasure.
