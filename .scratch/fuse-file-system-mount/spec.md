# Mount Full-residency File Systems with FUSE

Status: ready-for-agent
Work: resolved
Milestone: M4 — Additional resource and client support
Dependencies: Existing File System public contracts; a Linux FUSE runtime and unprivileged mount support for real-mount qualification; fuse3 0.10 with its Tokio runtime.

## Problem Statement

Users and application developers can operate an offline-first File System only through its Rust or browser APIs. They cannot yet expose a Device-local Replica as an ordinary operating-system directory. This prevents existing tools from reading and editing locally retained files without a bespoke integration.

A mount must preserve the current ownership boundaries. The File System owns paths, metadata, content, persistence, history, and reconciliation. Storage retains Resource authorization and Selection. The caller retains transport and synchronization lifecycle. A FUSE adapter must not become another persistence engine or an authorization bypass.

The first implementation must have an honest support boundary. It will mount only Full-residency File Systems on Linux. Passthrough content retrieval, live kernel-cache notifications, and complete POSIX behavior require separate work.

## Solution

Add a native FUSE adapter crate that mounts an already opened File System through fuse3's asynchronous path-based API. Applications can then use ordinary filesystem operations for local files and directories while the File System remains the only owner of persisted content and metadata.

The adapter accepts the File System from its caller. The caller opens the Resource, establishes Full Residency, supplies authorized synchronization, and controls shutdown. The adapter does not silently change Residency, open network connections, select Resources, or install a permissive OperationControl.

The initial mount supports ordinary file and directory work: lookup, attributes, open, close, offset reads and writes, create, truncate, append, mkdir, directory reads, unlink, empty-directory removal, no-replace rename, flush, and fsync. Writes remain durable File System Content Revisions. Reads and metadata lookups require locally retained content and return an operating-system error when it is absent.

Unsupported operations fail explicitly. The implementation and its documentation must not claim complete POSIX compatibility, Passthrough support, multi-Device convergence qualification, or support for another operating system.

## User Stories

1. As an application developer, I want to mount an opened File System at a local directory, so that existing applications can use its files without learning the File System API.
2. As a user, I want to read a locally retained file through a mount without a network connection, so that ordinary tools work offline.
3. As a user, I want to create and edit a file through a mount, so that the File System persists my work as a new Content Revision.
4. As a user, I want offset reads and writes to preserve unrelated bytes, so that random-access applications can use mounted files.
5. As a user, I want to truncate a file through open flags or a size attribute, so that editors and command-line tools can replace file contents safely.
6. As a user, I want append writes to add bytes at the current end of a file, so that log-style applications work through the mount.
7. As an application developer, I want exclusive file creation to reject an existing path, so that create-new operations cannot overwrite another caller's file.
8. As a user, I want to create and list directories, so that I can organize mounted content with ordinary tools.
9. As a user, I want unlink to remove files and rmdir to remove only empty directories, so that directory deletion cannot silently orphan children.
10. As a user, I want no-replace rename for files and directory trees, so that I can change paths without overwriting an existing destination.
11. As an application developer, I want file identity to remain stable across supported writes and renames, so that kernel and handle state can use the existing File Entry identity.
12. As a user, I want a successful write or fsync to mean that the corresponding Content Revision reached the File System's durable commit boundary, so that mount success does not weaken existing persistence behavior.
13. As a user, I want a restarted application to mount the same Replica and read its prior content, so that mounting does not require a separate cache or export step.
14. As an application developer, I want explicit unmount and shutdown behavior, so that mount threads, open handles, and synchronization tasks do not retain the managed copy unexpectedly.
15. As an application developer, I want a mount attempt to reject non-Full or incomplete Residency before exposing the mountpoint, so that the initial support boundary is enforced rather than discovered during random writes.
16. As an application developer, I want mounting to leave Residency unchanged, so that mounting cannot evict content or silently make a read-only Replica writable.
17. As a Node operator, I want Storage, Device approval, Resource Selection, and OperationControl policy to remain with the calling application, so that FUSE cannot bypass Resource authorization.
18. As a Node operator, I want ordinary mount operations to require no network access, so that offline behavior is deterministic.
19. As a user, I want missing local content and transport-independent storage failures reported as operating-system errors, so that applications do not receive false empty files or successful writes.
20. As an application developer, I want stale Content Revisions reported as stale handles and invalid paths reported as invalid arguments, so that errors retain actionable meaning across the FUSE boundary.
21. As an application developer, I want non-UTF-8 names rejected explicitly, so that the adapter cannot lossy-convert two distinct operating-system names into one File System path.
22. As a user, I want unsupported symlinks, hard links, xattrs, special files, and metadata changes to return an unsupported-operation error, so that the mount does not pretend to persist behavior that the File System does not model.
23. As a user, I want Unix permission checks to use the mount owner's fixed identity and the existing mode values, so that local access checks do not become a second Resource authorization system.
24. As an application developer, I want short attribute, entry, and data cache lifetimes, so that a later local or synchronized Content Revision is not hidden by a long-lived kernel cache.
25. As an application developer, I want concurrent mount operations to preserve revision integrity, so that two local writers cannot corrupt content or publish an unintended revision.
26. As a user, I want a conflicting external revision or unsupported open-file transition to produce an error rather than silent data loss, so that limited POSIX behavior remains safe.
27. As a maintainer, I want one adapter over the existing File System contracts, so that native and shared persistence continue to use the same reconciliation and commit paths.
28. As a maintainer, I want new core operations to use the existing Content Revision and snapshot formats, so that current Replicas require no data migration and updated peers do not need a new sync framing version for this feature.
29. As a maintainer, I want real kernel-mount acceptance tests, so that a trait implementation or unit test is not mistaken for a usable filesystem.
30. As a user, I want documentation to name Linux and Full Residency as the only qualified initial environment, so that I do not infer Passthrough, macOS, Windows, browser, or complete multi-Device support.

## Implementation Decisions

- Add one sibling adapter crate named `file-system-fuse`. Keep the existing `file-system` crate independent of FUSE and browser-incompatible native mount dependencies.
- Use fuse3 0.10 with default features disabled and only the Tokio runtime and required unprivileged-mount support enabled. The repository's Rust 1.100 minimum satisfies the library's MSRV.
- Start with fuse3's path-based `PathFilesystem` interface. It matches the File System's path-oriented asynchronous API and avoids a second adapter-owned inode table. Do not use its hard-link behavior because hard links are unsupported. Change to the raw inode interface only when a real mount test demonstrates a limitation in supported behavior.
- Expose an adapter that accepts an opened generic `FileSystem<PeerId>`. Do not let the adapter open a Catalog or Resource implicitly. This permits direct, shared, and Storage SDK construction without duplicating those policies in the adapter.
- Return a mount/session guard with explicit shutdown. Keep the backing root and mountpoint distinct. Stop request handling and close adapter-owned handles before releasing the File System; the caller remains responsible for stopping its synchronization executor.
- Restrict the initial adapter to Linux. Compile-time target gates and package documentation must prevent accidental inference of browser, Windows, or macOS qualification.
- Require Full Residency for the root and every existing entry before mounting. Verify that all current content is locally readable. Reject an incomplete Replica before mount and never call a Residency-changing operation as a mount side effect.
- Keep authorization and synchronization outside the adapter. Do not construct a default OperationControl. Existing local operations need no transport; the caller may independently synchronize and fetch Content Revisions through the existing authorized interfaces.
- Synthesize the root directory attributes. For other entries, use the existing File Entry identity, kind, and mode. Use the mount owner's numeric UID and GID and fixed mount-time timestamps. Do not interpret owner/group strings or UUIDv7 timestamps as POSIX ownership or timestamp contracts.
- Use local permission checks and do not enable `allow_other` by default. File modes are local metadata, not folder ACLs, Resource grants, Device approval, or Selection.
- Convert operating-system parent and name components to the File System's relative UTF-8 path contract without lossy conversion. Reject non-UTF-8 names with `EILSEQ`; reject reserved, absolute, parent-relative, and otherwise invalid paths with `EINVAL`.
- Preserve a small explicit errno mapping: not found to `ENOENT`, wrong entry types to `ENOTDIR` or `EISDIR`, duplicate destinations to `EEXIST`, non-empty directories to `ENOTEMPTY`, Passthrough writes to `EROFS`, stale revisions to `ESTALE`, cancellation to `ECANCELED`, denied operations to `EACCES`, and unavailable content or unclassified storage failures to `EIO`. Preserve a more specific underlying I/O error when available.
- Make `LocalFileHandle` own a cheap File System clone rather than borrow the File System. This removes the self-referential handle-table problem in a `'static` FUSE adapter and keeps revision-bound local I/O on the existing handle seam. This changes a Rust lifetime, not persisted data or synchronization frames.
- Add a local content-length query at the existing content boundary and expose it through the public File System API. Native and shared stores can use retained revision-file metadata; the browser store can use its retained blob size. Do not read a whole file for `getattr`, and do not add a persisted size field or metadata migration for this feature.
- Add an atomic create-if-absent operation in the File System's existing serialized mutation path. Use it for FUSE create and exclusive-create behavior rather than performing an unprotected adapter lookup followed by a write.
- Add a revision-checked truncate operation in the same staged Content Revision path used by offset writes. Preserve File Entry identity, require Full Residency, support zero-fill extension, sync the staged content, and publish metadata only after the content stage succeeds. Implement the internal content operation for every existing backend so the core contract remains portable even though FUSE is native-only.
- Reuse the existing handle read/write/scan operations for mounted I/O. Store open-file and open-directory state in bounded handle tables with unique kernel handle IDs. Serialize revision updates per open handle and coordinate same-file mount mutations sufficiently to avoid adapter-generated stale-write races.
- Snapshot directory entries at `opendir` and use stable indices for directory offsets. Include `.` and `..` without creating File System entries for them. Close directory handles exactly once at `releasedir`.
- Check entry types before unlink and rmdir. Reject rmdir when the existing public list operation returns children. Serialize adapter check-and-delete sequences against other mount mutations, but do not claim global atomicity against independent File System callers or remote Replicas.
- Map rename only when the destination does not exist. Return `EEXIST` for an occupied destination and `ENOSYS` for rename exchange or other unsupported flags. Do not emulate atomic replacement with a separate delete followed by rename.
- Handle append by combining the current content length with the handle's revision-checked write path. A conflicting revision must fail rather than append at an unchecked offset.
- Treat every successful core write as already synchronized to the existing durable content boundary. `flush` and `release` must not invent a second commit protocol; `fsync` can complete after the relevant admitted File System operations have completed.
- Disable writeback caching and use zero attribute, entry, and data cache lifetimes for the initial implementation. Do not add a public remote-change notification API or kernel invalidation optimization yet. Revalidate path and revision state through the File System on each operation.
- Return `ENOSYS` for unsupported metadata changes and file types. Mode creation uses the existing defaults; chmod, chown, timestamp updates, xattrs, POSIX lock propagation, symlinks, hard links, and special files are not persisted.
- Preserve the existing persisted File Entry JSON and filesystem sync protocol version. Owned handles, content-length queries, create-if-absent, and truncate are API and internal content-boundary additions only. Existing stores open without migration, and truncate or create writes reach older peers as ordinary new Content Revisions. Older peers remain unable to originate the new local API calls, which requires no wire negotiation.
- Update the crate overview, public Rustdoc, and current support documentation with the exact supported operations, mount prerequisites, error behavior, and qualification limits. Do not present the accepted later Passthrough design as implemented behavior.

## Testing Decisions

- Use one primary acceptance seam: ordinary operating-system filesystem calls against a real Linux kernel mount created through the public adapter API. Test external behavior, not FUSE callback invocation counts or private handle tables.
- Build the mount fixture on the existing public File System open and Residency APIs. Use separate temporary backing and mount roots, and always unmount and remove both roots during cleanup. A compile-only test or mocked `PathFilesystem` implementation is not mount qualification.
- Cover file creation, exclusive creation, overwrite, offset read/write, append, truncate smaller and larger, sparse offset behavior, flush, fsync, close, restart persistence, and stable File Entry identity.
- Cover mkdir, sorted directory iteration and offsets, `.` and `..`, unlink, non-empty rmdir rejection, directory-tree rename, occupied-destination rename rejection, and open-handle cleanup.
- Cover invalid and non-UTF-8 names, wrong entry types, missing content, stale revisions, read-only policy errors, unsupported operations, and exact public errno mapping where the operating system exposes it.
- Cover multiple open handles and concurrent readers/writers through OS operations. Assert final bytes and revision integrity rather than relying only on successful return codes.
- Reject mounting a default Passthrough File System, a nested Passthrough rule, and an incomplete Full Replica. Prove that a failed mount does not change Residency or evict content.
- Exercise synchronized metadata and content through the existing native Storage filesystem transport integration seam for final acceptance. Use the existing in-memory transport only for focused protocol fault cases; it is not sufficient evidence for authorized native synchronization or the M4 multi-Device workflow.
- While a Full Replica is mounted, apply local and remote revisions, make the new content locally available through the existing authorized path, and prove subsequent OS operations observe it with the configured cache policy. Do not claim live Passthrough fetching or kernel notification from this test.
- Test unmount with open files, active directory handles, pending synchronization, learned policy denial, and managed-copy retirement. Prove later operations fail cleanly and no background task prevents root cleanup.
- Use the existing public File System integration tests as the secondary seam for owned handles, content length, create-if-absent, and truncate. Exercise native, shared, and browser content implementations where applicable. Do not add test-only content or metadata interfaces.
- Extend existing fault-injection tests around truncate and handle ownership. Prove write, sync, publish, and metadata failures preserve the previous visible Content Revision and clean abandoned staging.
- Prove compatibility by reopening pre-feature persisted data without migration and by synchronizing new create/truncate revisions with the existing versioned protocol. Do not add a schema or protocol bump when the wire shape is unchanged.
- Use current File System local-operation, persistence, integrity, cancellation, admission, and native Storage filesystem transport tests as prior art.
- Run focused adapter and File System tests first, then formatting, the appropriate feature-powerset checks, focused and workspace-relevant Clippy, CRAP checks, and documentation checks. Run real mount tests in an environment with `/dev/fuse` and unprivileged mount support. Record skipped or unavailable kernel tests explicitly and never report them as passed qualification.

## Out of Scope

- Passthrough mounts, fetch-on-read, fetch-on-open, or network access from FUSE callbacks.
- Automatic Full-content materialization after remote metadata import.
- Public local/remote change subscriptions, kernel entry or inode notifications, and cache-performance tuning.
- A guarantee that the M4 two-Device offline convergence or Full/Passthrough workflows are complete.
- Windows, browser, macOS, FreeBSD, or other platform qualification. Later platform work may reuse the adapter contract but needs its own runtime evidence.
- Complete POSIX semantics, including atomic rename replacement, rename exchange, open-after-unlink, open-file traversal after rename, memory-map behavior, hard links, symlinks, special files, xattrs, and distributed file locking.
- Persistent UID, GID, atime, mtime, ctime, chmod, or chown support.
- A new persisted metadata schema, synchronized size field, storage migration, or filesystem sync protocol version.
- Write coalescing, delayed revision publication, Content Revision garbage collection, or other performance redesign before real mount measurements establish the need.
- Automatic transport construction, Device approval, Resource Selection, identity handling, or permissive authorization defaults.
- Mounting over the backing storage root or exposing internal metadata and content files through the File System namespace.

## Further Notes

The investigation found no existing FUSE dependency or tracked FUSE specification. The current File System already supplies stable File Entry identities, Content Revisions, local persistence, explicit remote content retrieval, policy-controlled synchronization, and substantial fault tests. The work here is an operating-system adapter plus a small set of core operations required by external mount behavior.

Passthrough remains a meaningful follow-up. Practical Passthrough lookup needs size metadata without fetching every file, and reads need an authorized provider plus explicit offline errors. Live kernel invalidation also needs a public change feed that includes remote imports. These are separate design and qualification tasks, not hidden requirements of the initial Full-residency mount.

External library facts were checked against fuse3 0.10.0 and fuser 0.18.0 primary documentation. fuse3 was selected for the first implementation because its asynchronous path interface matches the existing core. No repository source or tests were changed or run during the investigation.
