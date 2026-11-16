# 04: device-service — join traits RPITIT, local IdP callbacks generic

**What to build:** The node-join owner and node-join identity traits return `impl Future + Send` on every async method (the shared boxed-future alias is deleted; consumers already use `&impl NodeJoinOwner`, so only the identity trait object in the server is affected — fixed in ticket 05). The local-IdP callbacks on the hosted control plane become generic closure fields (`F: Fn(I) -> Fut, Fut: Future<Output = ...> + Send`), removing the `Arc<dyn Fn(...) -> Pin<Box<dyn Future>>>` fields; all in-crate call sites updated. Implementations on the security protocol updated.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] device-service compiles with no `Pin<Box<dyn Future` and no async `dyn` trait fields
- [ ] Package tests pass; clippy/rustfmt clean
