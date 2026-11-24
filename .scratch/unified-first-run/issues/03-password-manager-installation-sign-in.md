# 03: Sign in to Password Manager with the selected Installation

**What to build:** Make **Sign in to sync** use the Installation created or selected during Node setup. Verify the OAuth client configuration against that Installation rather than assuming the seeded development client exists. Keep local vault use independent of sign-in.

**Blocked by:** 01 (Create an Installation through Portless), 02 (Join an existing Installation as a Device).

**Status:** ready-for-agent

- [ ] The default Password Manager dev URL can complete sign-in and establish a session with either setup path, without manual environment overrides or seeded OAuth clients.
- [ ] Sign-in and Device approval stay separate. Sign-in does not unlock, replace, or migrate local vault data.
- [ ] Real-backend browser tests verify sign-in for both setup choices and local vault use without sign-in.
