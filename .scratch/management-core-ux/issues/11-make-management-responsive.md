# 11: Make management responsive

**What to build:** Administrators can complete core workflows on small screens without clipped actions or records. Navigation remains keyboard accessible and retains its selected state.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] At 320, 390, 768, and 1440 CSS pixels, primary forms and actions remain visible and operable without page-level horizontal clipping.
- [ ] Small-screen navigation has an appropriate open and closed pattern, and the sidebar preference survives routes and reloads.
- [ ] Wide tables and the Configuration control do not obscure actions or records.
- [ ] Keyboard focus remains visible and predictable when navigation opens, closes, or changes route.
- [ ] Real-backend browser checks cover Application, User, and Device workflows at desktop and mobile sizes.

The app stores separate desktop and small-screen navigation preferences. Existing data, APIs, and wire protocols do not change.
