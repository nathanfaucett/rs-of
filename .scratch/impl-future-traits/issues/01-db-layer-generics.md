# 01: ofdb engine/history layer — generic futures instead of boxed futures

**What to build:** In the ofdb engine and SQL-history crates, remove every `Pin<Box<dyn Future>>` signature. The stage helper that wraps engine closures returns the caller's future unwrapped; the Engine read/mutate transaction bounds take a second type parameter (`F: FnOnce(...) -> Fut, Fut: Future<Output = ...> + MaybeSend`) instead of naming a boxed future type; the history pre-apply check alias becomes a generic bound, and its one internal cast site stops boxing. Behavior is unchanged on native and wasm32 (`MaybeSend` keeps browser builds working).

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] ofdb-sql-engine and ofdb-sql-history contain no `BoxFuture` / `Pin<Box<dyn Future` references
- [ ] Both crates compile and their unit tests pass
- [ ] Clippy and rustfmt clean for both crates
