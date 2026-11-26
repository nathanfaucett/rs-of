# 06: Restore OAuth Client lifecycle

**What to build:** An authorized administrator can create an OAuth Client, open its registration, and return to the created-client list without losing the client ID.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Work status:** resolved

- [x] Opening a newly created client loads its detail through the existing authorized lookup contract.
- [x] Created-client metadata survives SPA navigation within the same application and authenticated session without persisting secrets.
- [x] Sign-out or session replacement clears the metadata, and reload limitations are explained.
- [x] Missing, unauthorized, and failed lookups have distinct guidance and useful retry or back actions.
- [x] Real-backend browser tests cover create, detail, Back, reload expectations, sign-out cleanup, and lookup failures.

## Comments

Registration permission does not grant registration read access. The IdP still requires the exact read grant and an Application-bound token for public Clients. The fresh seeded fixture grants only Client read in the login Application. There is no policy backfill, contract change, or data migration. The app uses the public registration path; it does not provision confidential or infrastructure Clients through that form. Session renewal remains in ticket 02.

- `e2e/app.e2e.ts` and `e2e/applications.e2e.ts` covered create, authorized detail and ID lookup, Back, reload limits, sign-out, session replacement, and distinct lookup failures. Owner HTTP tests confirmed Application-bound Client read isolation.
