# `file-system-fuse`

Linux read-only FUSE adapter for an already opened `file-system::FileSystem`. The caller owns Catalog and Resource opening, authorization, synchronization, operation policy, and Residency. The adapter never opens transport or changes Residency.

## Current support

- Linux only, using fuse3 0.10 with Tokio and unprivileged `fusermount3` mounts. `/dev/fuse` and working unprivileged mount support are required.
- The root and every existing entry must have Full Residency. Every regular file's current Content Revision must be locally retained before the mount is exposed. Passthrough and incomplete Replicas are rejected without changing Residency or content.
- The mountpoint must be an existing empty directory distinct from the backing root. Native callers must re-establish Full Residency after reopening a native File System; mounting does not do it for them.
- Lookup, attributes, read-only file open, offset reads, flush/release, directory open, stable-offset directory reads including `.` and `..`, and directory release are supported. Attribute and entry TTLs are zero; file reads use direct I/O and writeback caching is disabled.
- Attributes use existing kind/mode values, the mount owner's numeric UID/GID, and one fixed mount-time timestamp. `allow_other` is off. Unix modes are local metadata, not Resource authorization.
- Unsupported mutation callbacks return `ENOSYS`. Opening with write or mutation flags returns `EROFS` so fuse3 cannot mistake the operation for supported stateless open. Missing local content returns `EIO`; stale handle revisions return `ESTALE`; non-UTF-8 names return `EILSEQ`; invalid/reserved paths return `EINVAL`.
- Call `MountGuard::unmount` and await it. Close operating-system file and directory handles first. Open-file shutdown, pending-write shutdown, and managed-copy retirement hardening belong to later work.

File creation/writing, truncate, directory mutation, rename, Passthrough retrieval, network callbacks, automatic synchronization, non-Linux platforms, and complete POSIX behavior are not supported.

The real kernel tests are ignored by default so environments without FUSE do not report a false pass. Run them explicitly when available:

```bash
cargo test -p file-system-fuse --test linux_mount -- --ignored --nocapture
```

Use the crate Rustdoc for the public mount API. Use `file-system` public integration tests for owned local handles and local content-length behavior.
