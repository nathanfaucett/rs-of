# `file-system`

Core offline-first filesystem crate for paths, file metadata, content, persistence, and reconciliation.

The crate owns stable file identities, content revisions, deletion tombstones, and Device-local Full/Passthrough residency. `of` owns Device approval, Resource Selection, authorization, and protocol-specific transport adapters.

Use the crate's Rustdoc and public integration tests as the API reference. `packages/file-system` contains the browser wrapper.

```bash
cargo test -p file-system --all-targets
```

Do not infer remote content availability or multi-Device synchronization from local filesystem operations. Passthrough reads require an authorized available Full provider, and revocation takes effect only when learned.

`LocalFileHandle` owns a cheap `FileSystem` clone and remains revision-bound. `FileSystem::local_content_len` queries only locally retained revision length through the native, shared, or browser content store; it does not fetch remote content or add persisted size metadata. `FileSystem::create_file` atomically creates an empty regular file with the existing default mode in the serialized mutation path. Native, shared, and browser stores use the same staged Content Revision durability boundary; no schema or sync-protocol change is required.

`FileSystem::truncate` requires Full Residency and the current Content Revision. It preserves File Entry identity, shrinks or zero-fills extension, and returns the new Content Revision. `LocalFileHandle::truncate` advances the owned handle to that revision so subsequent writes use the new revision. Content is staged, synced, and published before metadata changes; failures before metadata commit preserve the prior visible revision and clean abandoned staging. Truncate uses the existing persisted File Entry and Content Revision formats and reaches peers through filesystem sync protocol version 1 as an ordinary revision.
