# 01 — Qualify browser SQL bindings

Status: needs-triage
Work status: open
Type: task
Blocked by: none

## Goal

Open durable local SQL through the actual JavaScript/WASM package in a real
browser. No desktop, HTTP SQL server, IdP, or app-specific type workaround.

## Scope

Primary changes belong in `ofdb/packages/sql` and `ofdb/crates/wasm`.
Use generated bindings to check names and number/BigInt signatures. Replace
ambiguous Text/Blob/UUID/integer conversion at the owner binding. Validate
parameters and results. Define handle/transaction disposal and commit outcomes.
Do not introduce a second storage engine or an HTTP-shaped local shim.

Make package and WASM artifacts consumable from a clean `of` checkout with pinned,
reproducible builds. No hard-coded sibling checkout paths. Record stack, worker,
browser, and bundler requirements. Shared fixes may also repair KV, but full KV
and offs adoption is not a prerequisite for this app.

## Acceptance

- Test through generated WASM and the public TS wrapper, not hand-written mocks.
- Round-trip base64-looking text, UUIDs, empty values, binary ciphertext, integer
  limits, and invalid values without guessing types or losing precision.
- Test atomic transaction commit/rollback and durable reopen after page restart.
- Cover quota/storage denial, stale concurrent transactions, interrupted commit,
  failed open, and disposal without leaked handles or tasks.
- A built browser app loads the package/WASM artifacts without Node/native imports.
- Document source-reviewed gaps separately from measured browser evidence.

## Comments

Planning found signature and value-conversion mismatches. Reproduce them before
fixing them. Existing Rust browser acceptance does not qualify these JS wrappers.
