# `file-system-fuse`

Linux FUSE adapter for an already opened `file-system::FileSystem`. The caller owns Catalog and Resource opening, authorization, synchronization, operation policy, and Residency. The adapter never opens transport or changes Residency.

## Current support

- Linux only, using fuse3 0.10 with Tokio and unprivileged `fusermount3` mounts. `/dev/fuse` and working unprivileged mount support are required.
- The root and every existing entry must have Full Residency. Every regular file's current Content Revision must be locally retained before the mount is exposed. Passthrough and incomplete Replicas are rejected without changing Residency or content.
- The mountpoint must be an existing empty directory distinct from the backing root. Native callers must re-establish Full Residency after reopening a native File System; mounting does not do it for them.
- Lookup, attributes, file open with optional `O_TRUNC`, size-only setattr, offset reads, ordinary and exclusive file creation, revision-checked offset and append writes, flush/fsync/release, mkdir, unlink, empty rmdir, directory open, stable-offset directory reads including `.` and `..`, and directory release are supported. Attribute and entry TTLs are zero; file I/O uses direct I/O and writeback caching is disabled. Ordinary create opens an existing regular file; exclusive create returns `EEXIST` for any existing entry without changing it. File and directory creation use the existing default modes. Mkdir creates one directory under an existing parent and returns `EEXIST` for either existing entry type. Unlink accepts only regular files. Rmdir accepts only directories and returns `ENOTEMPTY` when the public directory listing has children. The synthetic root cannot be deleted.
- Attributes use existing kind/mode values, the mount owner's numeric UID/GID, and one fixed mount-time timestamp. `allow_other` is off. Unix modes are local metadata, not Resource authorization.
- Every admitted create, write, or truncation finishes the existing staged-content sync, publication, and metadata operation before success. Open-time and size-attribute truncation preserve File Entry identity and advance the owned handle revision. Shrinking removes trailing bytes; extension zero-fills new bytes, including extension from zero length. Flush, fsync, and release wait for relevant handle operations; they do not add a second commit protocol. Directory deletion uses the existing File System deletion tombstones. Truncate adds no second Tombstone or Content Revision mechanism and no schema or sync-protocol change. Duplicate creation returns `EEXIST`; missing entries return `ENOENT`; wrong types return `EISDIR` or `ENOTDIR`; non-empty directories return `ENOTEMPTY`; invalid input returns `EINVAL`; Passthrough mutations return `EROFS`; stale revisions return `ESTALE`; cancellation returns `ECANCELED`; denied operations return `EACCES`; unavailable content and unclassified storage failures return `EIO`. Specific underlying OS errors are preserved. Non-UTF-8 names return `EILSEQ`; invalid/reserved paths return `EINVAL`.
- Directory entries are snapshotted at `opendir`, so later namespace mutations do not change an existing directory stream. File and directory handles share unique IDs and exactly-once adapter release. Call `MountGuard::unmount` and await it. Close operating-system file and directory handles first. Open-file shutdown, active-directory shutdown, pending-write shutdown, and managed-copy retirement hardening belong to later work.

Create, open-time truncation, size-only setattr, mkdir, unlink, and rmdir share adapter-local mutation coordination, including the rmdir check-and-delete sequence. This does not provide atomicity against independent File System callers or other Replicas. A conflicting external Content Revision fails with `ESTALE` instead of writing or truncating at an unchecked revision.

Only the size attribute is mutable. Requests containing mode, ownership, timestamp, or platform flag changes return `ENOSYS` before any requested size change. FUSE lock-owner context is not a persisted attribute. Editor-style truncate-and-write replacement is supported; atomic rename replacement, other open-file transitions, recursive deletion, open-after-unlink guarantees, Passthrough retrieval, network callbacks, automatic synchronization, non-Linux platforms, and complete concurrent-writer POSIX behavior are not supported.

The real kernel tests are ignored by default so environments without FUSE do not report a false pass. Run them explicitly when available:

```bash
cargo test -p file-system-fuse --test linux_mount -- --ignored --nocapture
```

Use the crate Rustdoc for the public mount API. Use `file-system` public integration tests for owned local handles and local content-length behavior.
