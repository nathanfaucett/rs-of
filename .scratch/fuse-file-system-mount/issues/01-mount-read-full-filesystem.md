# 01: Mount and read a Full-residency File System

**What to build:** An application can mount an already opened Full-residency File System on Linux and use ordinary operating-system calls to look up attributes, read files, and iterate directories. The new `file-system-fuse` crate owns only the FUSE adapter and mount lifecycle. The existing `file-system` crate gains owned local handles and a local content-length query needed by that adapter. Mounting rejects incomplete or non-Full Replicas without changing Residency, and a real kernel-mount test proves offline reads, restart persistence, and clean unmount.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Scope limits:** No file creation or mutation, Passthrough retrieval, network callbacks, automatic synchronization, non-Linux qualification, or complete POSIX claim.

**Current evidence:** The File System already provides local reads, stable File Entry identities, Content Revisions, persistence, and public integration tests. No FUSE adapter or kernel-mount qualification exists.

- [ ] Add a separate native `file-system-fuse` crate using fuse3 0.10 with default features disabled and only the Tokio runtime and required unprivileged-mount support enabled; keep FUSE dependencies out of `file-system`.
- [ ] The adapter accepts an opened generic File System and returns an explicit mount/session guard; it does not open a Catalog or Resource, construct transport, install an OperationControl, or change Residency.
- [ ] Mount validates Full Residency for the root and every existing entry and verifies local content availability before exposing the mountpoint; failure leaves Residency and retained bytes unchanged.
- [ ] `LocalFileHandle` owns a cheap File System clone rather than borrowing it, while preserving existing revision-bound read, write, scan, and close behavior for current callers.
- [ ] A public local content-length query uses the existing content boundary in native, shared, and browser stores without reading the whole file, adding a persisted size field, or changing sync framing.
- [ ] Linux users can perform lookup, getattr, open, offset read, opendir, stable-offset readdir, and close operations through a real FUSE mount, including synthesized root, `.` and `..` entries.
- [ ] Path conversion preserves the relative UTF-8 contract and rejects non-UTF-8, absolute, reserved, and parent-relative paths without lossy aliases.
- [ ] Initial errno mapping covers missing entries, wrong types, invalid input, unavailable content, denied operations, cancellation, stale revisions, and underlying I/O errors without false success or false empty files.
- [ ] Attributes use existing kind and mode values, mount-owner UID/GID, and fixed mount-time timestamps; `allow_other` is off by default and Unix modes remain separate from Resource authorization.
- [ ] Attribute, entry, and data cache lifetimes are zero and writeback caching is disabled; unsupported mutation operations return `ENOSYS` until their vertical slices land.
- [ ] Backing and mount roots remain distinct, and unmount closes adapter-owned handles and request processing without leaving a task that prevents managed-copy cleanup.
- [ ] Real Linux mount tests cover offline file reads, directory iteration, restart persistence, failed Passthrough and incomplete-Full mounts, and clean unmount; unavailable kernel tests are recorded as unrun rather than passed.
- [ ] Focused File System and adapter tests, formatting, relevant Clippy, and documentation checks pass; crate Rustdoc and current support text state the exact read-only increment and Linux/Full limits.

# Comments
