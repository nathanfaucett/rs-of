# 06: storage server/service — authorizer, stream, and content-reader callbacks generic

**What to build:** The frame authorizers (database and filesystem transports), the local replication resolve policy, the additional-stream handler, and the scoped file-system content reader stop being `Arc<dyn Fn(...) -> Pin<Box<dyn Future>>>`. The protocol handlers and the scoped runtime become generic over the closure/future pair (`F: Fn(...) -> Fut, Fut: Future<Output = ...> + Send`), with construction sites (including the local replication authorizer, whose constructor already takes a generic closure) updated.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] storage-server and storage-service compile with no boxed/dyn async callback fields
- [ ] Storage-server tests pass, including the database execution / replication admission tests; clippy/rustfmt clean
