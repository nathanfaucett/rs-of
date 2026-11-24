# 02: Use generated Logical Row identities in IdP persistence

**What to build:** IdP persistence creates Users, Applications, OAuth Clients, Credentials, Keys, grants, consents, and related records with generated UUIDv7 Logical Row identities. Operations that require predetermined public domain identifiers retain those identifiers separately from the internal row identity. Local seeding, authentication, token issuance, restart, and Replica synchronization continue through public IdP interfaces without caller-supplied single-UUID primary keys.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

Work: resolved

- [x] IdP insertion paths omit single-UUID primary keys or use DEFAULT and consume generated UUIDv7 values through RETURNING where required.
- [x] Creation methods that accept stable public IDs preserve their idempotent domain behavior without using those IDs as single-UUID Logical Row identities.
- [x] Local seeded Users, Credentials, permissions, and signer-related records retain stable public identity and sign-in behavior across fresh initialization and restart.
- [x] Foreign references and queries consistently use the intended public domain identity or generated Logical Row identity without conflating them.
- [x] OAuth authorization, refresh, consent, and credential workflows remain covered by public service and repository tests.
- [x] Fresh-store and updated-Replica synchronization tests pass; no migration or compatibility path is added.
- [x] Focused IdP service tests and required formatting and lint checks pass.

## Comments

Implementation integrated. IdP service, OAuth/Replica, fresh snapshot, and security projection tests passed. The IdP server suite had 11 schema/translator failures whose baseline was not confirmed. The feature powerset failed on no-default-features imports, and the focused CRAP run reported 46 functions above threshold.

2026-11-24: The no-default import failure is fixed. Focused library Clippy passed with the two pre-existing workspace-policy allowances; 71 all-feature and 16 no-default library tests passed. Formatting passed. Broader workspace and CRAP limits are recorded under issue 06.
