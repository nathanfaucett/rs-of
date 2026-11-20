# `file-system-fuse`

Linux FUSE adapter for an already opened `file-system::FileSystem`. The caller owns Catalog and Resource opening, authorization, synchronization, operation policy, and Residency. The adapter never opens transport or changes Residency.

## Current support

- Linux only, using fuse3 0.10 with Tokio and unprivileged `fusermount3` mounts. `/dev/fuse` and working unprivileged mount support are required.
- The root and every existing entry must have Full Residency. Every regular file's current Content Revision must be locally retained before the mount is exposed. Passthrough and incomplete Replicas are rejected without changing Residency or content.
- The mountpoint must be an existing empty directory distinct from the backing root. Native callers must re-establish Full Residency after reopening a native File System; mounting does not do it for them.
- Lookup, attributes, file open, offset reads, ordinary and exclusive file creation, revision-checked offset and append writes, flush/fsync/release, directory open, stable-offset directory reads including `.` and `..`, and directory release are supported. Attribute and entry TTLs are zero; file I/O uses direct I/O and writeback caching is disabled. Ordinary create opens an existing regular file; exclusive create returns `EEXIST` for any existing entry without changing it. Creation uses the existing default file mode.
- Attributes use existing kind/mode values, the mount owner's numeric UID/GID, and one fixed mount-time timestamp. `allow_other` is off. Unix modes are local metadata, not Resource authorization.
- Every admitted create/write finishes the existing staged-content sync, publication, and metadata operation before success. Flush, fsync, and release wait for relevant handle operations; they do not add a second commit protocol. Duplicate exclusive creation returns `EEXIST`; missing entries return `ENOENT`; wrong types return `EISDIR` or `ENOTDIR`; invalid input returns `EINVAL`; Passthrough writes return `EROFS`; stale revisions return `ESTALE`; cancellation returns `ECANCELED`; denied operations return `EACCES`; unavailable prior content and unclassified storage failures return `EIO`. Specific underlying OS errors are preserved. Non-UTF-8 names return `EILSEQ`; invalid/reserved paths return `EINVAL`.
- Call `MountGuard::unmount` and await it. Close operating-system file and directory handles first. Open-file shutdown, pending-write shutdown, and managed-copy retirement hardening belong to later work.

Truncate, directory mutation, rename, Passthrough retrieval, network callbacks, automatic synchronization, non-Linux platforms, and complete concurrent-writer POSIX behavior are not supported. A conflicting external Content Revision fails with `ESTALE` instead of writing at an unchecked offset.

The real kernel tests are ignored by default so environments without FUSE do not report a false pass. Run them explicitly when available:

```bash
cargo test -p file-system-fuse --test linux_mount -- --ignored --nocapture
```

Use the crate Rustdoc for the public mount API. Use `file-system` public integration tests for owned local handles and local content-length behavior.
