# 05: Clarify Identity Boundary in Documentation

**What to build:** Documentation explicitly states that "Identity" is split into two concepts: user identity (OAuth/OIDC, managed by IdP-service) and device endpoint identity (Iroh keys, managed by Device-service). APIs must not conflate the two.

**Blocked by:** 01 (foundation), but can be done in parallel with any other ticket

**Status:** ready-for-agent

**Acceptance criteria:**
- [ ] `docs/architecture.md` or `GLOSSARY-MAP.md` clarifies the two identity types
- [ ] Service ownership matrix in docs reflects the identity split
- [ ] OpenAPI contracts don't mix identity types (e.g., don't use "user" where "device endpoint" is meant)

**Blocked by:** 01
