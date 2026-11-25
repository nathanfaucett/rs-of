# 06: Restore OAuth Client lifecycle

**What to build:** An authorized administrator can create an OAuth Client, open its registration, and return to the created-client list without losing the client ID.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Opening a newly created client loads its detail through the existing authorized lookup contract.
- [ ] Created-client metadata survives SPA navigation within the same application and authenticated session without persisting secrets.
- [ ] Sign-out or session replacement clears the metadata, and reload limitations are explained.
- [ ] Missing, unauthorized, and failed lookups have distinct guidance and useful retry or back actions.
- [ ] Real-backend browser tests cover create, detail, Back, reload expectations, sign-out cleanup, and lookup failures.
