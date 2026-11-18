# Synthetic password-manager fixtures

These hand-written JSON files are research inputs for a possible `personal-json-v1` importer. They are not live exports and do not prove importer behavior.

All names, passwords, TOTP seeds, identifiers, card values, and key text are synthetic. SSH material is deliberately nonfunctional. Do not enter fixture values into real accounts.

- `supported.json`: representative records, field states, folders, history, and Unicode values.
- `cases.json`: named patches and expected outcomes for future acceptance tests.
- `sources.json`: public first-party source references, pinned revisions, retrieval dates, and content hashes.

Import is outside the password manager's initial platform-reference scope. If importer work begins, define its public contract and test round trips, duplicate handling, malformed input, destination preservation, rollback, and secret-safe diagnostics against the real application. Structural JSON checks and source research alone are not application validation.
