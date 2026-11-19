# 02: Use generated Logical Row identities in IdP persistence

**What to build:** IdP persistence creates Users, Applications, OAuth Clients, Credentials, Keys, grants, consents, and related records with generated UUIDv7 Logical Row identities. Operations that require predetermined public domain identifiers retain those identifiers separately from the internal row identity. Local seeding, authentication, token issuance, restart, and Replica synchronization continue through public IdP interfaces without caller-supplied single-UUID primary keys.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] IdP insertion paths omit single-UUID primary keys or use DEFAULT and consume generated UUIDv7 values through RETURNING where required.
- [ ] Creation methods that accept stable public IDs preserve their idempotent domain behavior without using those IDs as single-UUID Logical Row identities.
- [ ] Local seeded Users, Credentials, permissions, and signer-related records retain stable public identity and sign-in behavior across fresh initialization and restart.
- [ ] Foreign references and queries consistently use the intended public domain identity or generated Logical Row identity without conflating them.
- [ ] OAuth authorization, refresh, consent, and credential workflows remain covered by public service and repository tests.
- [ ] Fresh-store and updated-Replica synchronization tests pass; no migration or compatibility path is added.
- [ ] Focused IdP service tests and required formatting and lint checks pass.
