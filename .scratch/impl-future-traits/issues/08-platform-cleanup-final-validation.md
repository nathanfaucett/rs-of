# 08: platform — delete the boxed-future alias, run full validation

**What to build:** With no remaining users, delete the `BoxFuture` alias from ofdb-platform (`MaybeSend`/`MaybeSync` stay). Then run the repo's full final validation: feature-powerset workspace tests (`cargo hack test --feature-powerset --workspace --all-targets` per the repo guide), clippy, rustfmt, and a wasm32-unknown-unknown build of the platform and file-system crates. Confirm by grep that zero `Pin<Box<dyn Future` occurrences remain in the workspace.

**Blocked by:** 01, 02, 03 (every former `BoxFuture` user must be migrated first)

**Status:** ready-for-agent

- [ ] `BoxFuture` is gone from ofdb-platform and the workspace has zero `Pin<Box<dyn Future` occurrences
- [ ] Feature-powerset workspace test suite passes
- [ ] Clippy and rustfmt clean; wasm32-unknown-unknown builds pass for platform and file-system
