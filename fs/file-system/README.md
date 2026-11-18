# `file-system`

Core offline-first filesystem crate for paths, file metadata, content, persistence, and reconciliation.

The crate owns stable file identities, content revisions, deletion tombstones, and Device-local Full/Passthrough residency. `of` owns Device approval, Resource Selection, authorization, and protocol-specific transport adapters.

Use the crate's Rustdoc and public integration tests as the API reference. `packages/file-system` contains the browser wrapper; [browser documentation](../../docs/browser.md) gives its local build command and support limits.

```bash
cargo test -p file-system --all-targets
```

Do not infer remote content availability or multi-Device synchronization from local filesystem operations. Passthrough reads require an authorized available Full provider, and revocation takes effect only when learned.
