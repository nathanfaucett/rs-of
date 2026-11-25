# 02: Restore session renewal and sign-in recovery

**What to build:** An expired session renews before API token exchange, or returns to Sign in with a safe return route. Users no longer remain in an authenticated shell that repeatedly fails.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] A controlled expired access token is renewed once under concurrent requests, and the original protected data loads.
- [ ] A stale refresh or renewal completion cannot replace or lock out a newer session.
- [ ] Failed renewal clears the invalid session, stops repeated failing requests, and returns to Sign in with a safe return location.
- [ ] Sign-in validation shows one concise, grammatically correct message for each actual problem.
- [ ] Tests use controlled expiry rather than waiting for real expiration; focused state and real-backend browser tests pass.
