# 03: Sign in to Password Manager with the selected Installation

**What to build:** Make **Sign in to sync** use the Installation created or selected during Node setup. Verify the OAuth client configuration against that Installation rather than assuming the seeded development client exists. Keep local vault use independent of sign-in.

**Blocked by:** 01 (Create an Installation through Portless), 02 (Join an existing Installation as a Device).

**Status:** ready-for-agent

- [ ] The default Password Manager dev URL can complete sign-in and establish a session with either setup path, without manual environment overrides or seeded OAuth clients.
- [x] Sign-in and Device approval stay separate. Sign-in does not unlock, replace, or migrate local vault data.
- [x] Real-backend browser tests verify sign-in for both setup choices and local vault use without sign-in.

## Handoff

Password Manager passes Create and Join with a read-only descriptor adapter and real owners. Ticket 04 owns default runtime qualification. No descriptor change: GET `/setup/node` on the unified origin returns `{state: "ready", idp, storage}` only after Create or Join completes. Other responses cannot start normal sign-in.

Allow the exact Password Manager origin on descriptor reads and the selected IdP discovery, token, Client metadata, and userinfo routes, and on Storage API operations. Support the relevant Authorization and Content-Type preflights. Do not expand setup-write access. For Join, the remote Installation must allow the Password Manager origin; CORS on the joining Node alone is insufficient.
