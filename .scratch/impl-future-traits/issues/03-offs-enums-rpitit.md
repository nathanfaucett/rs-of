# 03: offs file system — closed backend enums + RPITIT root/content/staged traits

**What to build:** The file-system `Root`, `ContentStore`, and `Staged` traits return `impl Future + MaybeSend` instead of boxed futures. Because native, shared, and (on wasm) browser backends coexist at runtime, unify them into closed enums held concretely by the managed copy and storage — no `Box<dyn RootObject>`, `Arc<dyn ContentObject>`, or `Box<dyn StagedObject>` remains. `stage()` returns the concrete staged enum via `impl Future`. The shared-copy builder closure takes the two-parameter generic form (`F: FnOnce(&str) -> Fut, Fut: Future<Output = ...>`). The file-future alias is deleted; test code that boxes match arms uses a small enum or plain async blocks.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] ofdb-file-system compiles for native and for wasm32-unknown-unknown
- [ ] No `dyn` root/content/staged objects and no boxed futures remain in the crate (including tests)
- [ ] File-system integration tests (shared filesystem, outbound send lifecycle) pass; clippy/rustfmt clean
