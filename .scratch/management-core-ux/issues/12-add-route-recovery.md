# 12: Add route titles and recovery navigation

**What to build:** Each management route identifies itself in the browser title, and error pages offer a clear route back. Normal route loads do not emit avoidable framework warnings.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Sign in, Dashboard, list routes, and detail routes have unique, useful document titles; detail titles identify the current record when loaded.
- [ ] The 404 page offers a Dashboard or safe Back action.
- [ ] Route loads use the framework-provided fetch where required and no longer emit the observed `window.fetch` warning.
- [ ] Browser checks cover titles, error recovery, and absence of new console errors; package checks pass.
