# 03: Sign in to Password Manager with the selected Installation

**What to build:** Make **Sign in to sync** use the Installation created or selected during Node setup. Verify the OAuth client configuration against that Installation rather than assuming the seeded development client exists. Keep local vault use independent of sign-in.

**Blocked by:** 01 (Create an Installation through Portless), 02 (Join an existing Installation as a Device).

**Status:** ready-for-agent

Work status: resolved

- [x] The default Password Manager dev URL can complete sign-in and establish a session with either setup path, without manual environment overrides or seeded OAuth clients.
- [x] Sign-in and Device approval stay separate. Sign-in does not unlock, replace, or migrate local vault data.
- [x] Real-backend browser tests verify sign-in for both setup choices and local vault use without sign-in.

## Comments

Real-backend Chromium verified sign-in and Storage access after Create and Join through the default Node descriptor, without an adapter. A vault record saved before sign-in remained available. A joined Installation must allow the Password Manager origin.
