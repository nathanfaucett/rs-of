# 02 — Build the app-owned local vault

Status: resolved
Work status: resolved
Type: task
Blocked by: 01

## Goal

Prove the password manager's data operations directly against local ofdb SQL,
before designing the shared collection or SDK setup.

## Scope

Replace app remote startup/SQL persistence in
`apps/password-manager/src/lib/storage/`. Keep `SecretSchema` and app schema
ownership. Open one explicit local vault identity; no remote User/Application
namespace or remote `ensureDatabase` call.

First record the reviewed vault format, established crypto dependency/facilities,
password derivation, key wrapping, authenticated encryption, nonce rules, format
version, key lifecycle, exposed metadata, and recovery limits. Do not build custom
cryptographic primitives. Encrypt all sensitive record fields before writing.
No durable plaintext search index or key persisted beside its ciphertext.

Use local transactions for schema setup and atomic mutation plus validated
snapshot. Implement encrypted export/verified restore. Define existing-only reopen
where evidence exists; report the limit when the browser deletes all evidence.
Do not silently repair incompatible schemas or replace missing acknowledged data.

## Acceptance

- Fresh create, lock/unlock, CRUD, and durable reopen work without network requests.
- Wrong keys, tampered ciphertext, malformed records, and incompatible schemas
  refuse access without publishing a partial/empty vault.
- Sensitive names, URLs, secret history, and values do not enter persistent
  plaintext, logs, or fixtures containing real credentials.
- Failed commit is not reported as saved; uncertain commit is not blindly replayed.
- Concurrent changes retain a safe recovery path; no silent credential overwrite.
- Export/restore reproduces complete records; invalid imports change nothing.
- Old remote SQL/filesystem data remains untouched and is not read as fallback.

## Comments

Keep the test-data-only warning. Functional encryption tests do not establish
production password-manager security or secure erasure of retained history.
