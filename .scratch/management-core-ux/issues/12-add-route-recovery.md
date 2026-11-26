# 12: Add route titles and recovery navigation

**What to build:** Each management route identifies itself in the browser title, and error pages offer a clear route back. Normal route loads do not emit avoidable framework warnings.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Work status:** resolved

- [x] Sign in, Dashboard, list routes, and detail routes have unique, useful document titles; detail titles identify the current record when loaded.
- [x] The 404 page offers a Dashboard or safe Back action.
- [x] Route loads use the framework-provided fetch where required and no longer emit the observed `window.fetch` warning.
- [x] Browser checks cover titles, error recovery, and no new console errors on Sign in, Dashboard, or the return from 404; package checks pass.
- [ ] Qualify absence of new console errors across all list and detail routes.

## Comments

- `e2e/app.e2e.ts` confirmed titles, 404 recovery, and no observed `window.fetch` warning. The console-error assertion did not cover every list and detail route.
