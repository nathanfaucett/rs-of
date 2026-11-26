# 02: Restore session renewal and sign-in recovery

**What to build:** An expired session renews before API token exchange, or returns to Sign in with a safe return route. Users no longer remain in an authenticated shell that repeatedly fails.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Work status:** resolved

- [x] A controlled expired access token is renewed once under concurrent requests, and the original protected data loads.
- [x] A stale refresh or renewal completion cannot replace or lock out a newer session.
- [x] Failed renewal clears the invalid session, stops repeated failing requests, and returns to Sign in with a safe return location.
- [x] Sign-in validation shows one concise, grammatically correct message for each actual problem.
- [x] Tests use controlled expiry rather than waiting for real expiration; focused state and real-backend browser tests pass.

## Comments

The real IdP accepts the password-login Client's refresh grant. The SDK then queries the public consent projection, which correctly excludes Clients without the authorization-code grant. Keep that authorization boundary. A narrow, opt-in SDK setting is needed for the app's fixed login Client, established by onboarding. No persisted-data, HTTP, or policy migration is required.

Concurrent token exchanges for different owner audiences can conflict when the IdP writes refresh grants. The app orders exchanges per session after sharing renewal. It still sends each audience separately and does not suppress owner errors. This does not qualify concurrent grant writes across independent clients or tabs.

- `e2e/app.e2e.ts` confirmed one renewal under concurrent requests, failed-renewal recovery on navigation and reload, safe return, and sign-in validation. `sessionAccess.test.ts` covered stale completions; the OIDC client tests passed.
