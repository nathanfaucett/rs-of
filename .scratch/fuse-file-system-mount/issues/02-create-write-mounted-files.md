# 02: Create and write mounted files

**What to build:** A user can create and edit regular files through the Linux mount. Exclusive creation cannot overwrite an existing entry, offset writes preserve unrelated bytes, append uses the current end, and successful writes reach the File System's existing durable Content Revision boundary. The core gains an atomic create-if-absent operation; the adapter wires it and the existing owned-handle write path through to real operating-system behavior.

**Blocked by:** 01: Mount and read a Full-residency File System

**Status:** ready-for-agent

**Scope limits:** No truncate, directory mutation, rename replacement, write coalescing, Content Revision garbage collection, Passthrough writes, or claim of complete concurrent-writer POSIX behavior.

**Current evidence:** The File System already stages whole and offset writes, syncs content before metadata publication, preserves File Entry identity, and has fault-injection tests. It lacks atomic create-if-absent and all FUSE write callbacks.

- [ ] A core create-if-absent operation executes in the existing serialized mutation path, requires Full Residency, returns `AlreadyExists` for any existing entry, and creates an empty regular File Entry with the existing default mode.
- [ ] FUSE create and exclusive-create use the atomic core operation rather than an unprotected adapter lookup followed by a write; duplicate creation returns `EEXIST` without changing existing content or revision.
- [ ] Ordinary create opens a usable owned handle, and offset writes preserve bytes before and after the written range while producing a distinct durable Content Revision with stable File Entry identity.
- [ ] Append writes use current content length plus the handle's revision-checked write path; a conflicting revision fails instead of writing at an unchecked offset.
- [ ] Write, flush, release, and fsync report success only after the relevant admitted File System operations complete; no second commit or persistence protocol is introduced.
- [ ] Write-side errno behavior covers directories, missing entries, invalid offsets or sizes, Passthrough policy, stale revisions, cancellation, denial, unavailable prior content, and underlying I/O failures.
- [ ] Real Linux mount tests cover create, exclusive create, overwrite, random-offset writes, append, flush, fsync, close, and restart persistence through ordinary OS calls.
- [ ] Existing native, shared, and browser API tests establish create-if-absent behavior where applicable; existing write fault tests continue to prove that failed publication leaves the previous visible revision intact.
- [ ] Pre-feature persisted data opens without migration and new creates/writes use the existing filesystem sync protocol version and appear to peers as ordinary Content Revisions.
- [ ] Current crate documentation marks file creation and offset/append writing as supported only within the Linux/Full mount contract and retains the concurrency and unsupported-operation limits.
- [ ] Focused core and real-mount tests, formatting, relevant feature and Clippy checks, and documentation checks pass with every qualification limit recorded.

# Comments
