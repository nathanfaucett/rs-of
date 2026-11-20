# 01: Mount and read a Full-residency File System

**What to build:** An application can mount an already opened Full-residency File System on Linux and use ordinary operating-system calls to look up attributes, read files, and iterate directories. The new `file-system-fuse` crate owns only the FUSE adapter and mount lifecycle. The existing `file-system` crate gains owned local handles and a local content-length query needed by that adapter. Mounting rejects incomplete or non-Full Replicas without changing Residency, and a real kernel-mount test proves offline reads, restart persistence, and clean unmount.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent
**Work:** in progress

**Scope limits:** No file creation or mutation, Passthrough retrieval, network callbacks, automatic synchronization, non-Linux qualification, or complete POSIX claim.

**Current evidence:** The File System already provides local reads, stable File Entry identities, Content Revisions, persistence, and public integration tests. No FUSE adapter or kernel-mount qualification exists.

- [x] Add a separate native `file-system-fuse` crate using fuse3 0.10 with default features disabled and only the Tokio runtime and required unprivileged-mount support enabled; keep FUSE dependencies out of `file-system`.
- [x] The adapter accepts an opened generic File System and returns an explicit mount/session guard; it does not open a Catalog or Resource, construct transport, install an OperationControl, or change Residency.
- [x] Mount validates Full Residency for the root and every existing entry and verifies local content availability before exposing the mountpoint; failure leaves Residency and retained bytes unchanged.
- [x] `LocalFileHandle` owns a cheap File System clone rather than borrowing it, while preserving existing revision-bound read, write, scan, and close behavior for current callers.
- [x] A public local content-length query uses the existing content boundary in native, shared, and browser stores without reading the whole file, adding a persisted size field, or changing sync framing.
- [x] Linux users can perform lookup, getattr, open, offset read, opendir, stable-offset readdir, and close operations through a real FUSE mount, including synthesized root, `.` and `..` entries.
- [x] Path conversion preserves the relative UTF-8 contract and rejects non-UTF-8, absolute, reserved, and parent-relative paths without lossy aliases.
- [x] Initial errno mapping covers missing entries, wrong types, invalid input, unavailable content, denied operations, cancellation, stale revisions, and underlying I/O errors without false success or false empty files.
- [x] Attributes use existing kind and mode values, mount-owner UID/GID, and fixed mount-time timestamps; `allow_other` is off by default and Unix modes remain separate from Resource authorization.
- [x] Attribute, entry, and data cache lifetimes are zero and writeback caching is disabled; unsupported mutation operations return `ENOSYS` until their vertical slices land.
- [x] Backing and mount roots remain distinct, and unmount closes adapter-owned handles and request processing without leaving a task that prevents managed-copy cleanup.
- [x] Real Linux mount tests cover offline file reads, directory iteration, restart persistence, failed Passthrough and incomplete-Full mounts, and clean unmount; unavailable kernel tests are recorded as unrun rather than passed.
- [x] Focused File System and adapter tests, formatting, relevant Clippy, and documentation checks pass; crate Rustdoc and current support text state the exact read-only increment and Linux/Full limits.

## Comments

- 2026-11-20: Implemented the owned handle, local content-length boundary, Linux/Full mount guard, read-only path-based FUSE session, validation, stable directory snapshots, errno/cache/attribute handling, and clean explicit unmount. Later mutation operations remain unsupported.
- Real `/dev/fuse` and `fusermount3` were available. Both ignored real-kernel tests passed explicitly: offline/offset reads, attributes, stable directory offsets, path/error behavior, stale/unavailable content, restart persistence, and clean unmount. All four mount-validation tests passed.
- `file-system` passed all targets and all four feature-powerset combinations. `file-system-fuse` passed its feature-powerset targets. Formatting, adapter Rustdoc, and `just docs-check` passed.
- Focused Clippy passed with the existing repository-wide `multiple_crate_versions` and `redundant_feature_names` lint classes allowed. The strict invocation also reports those pre-existing dependency/feature lints. The direct WASM browser check is blocked by an existing `ofdb-kv` missing-`io` error outside this ticket.
- Focused LLVM coverage included real mounts. CRAP reported 13 findings: 12 existing core functions and the adapter's branch-heavy errno mapper (46.7% branch coverage). This is recorded, not claimed as a clean CRAP result.
- Compatibility decision: the handle change removes a Rust lifetime only. No persisted field, schema migration, or filesystem sync-framing/version change was added. Existing native/shared/browser revision formats remain unchanged.
