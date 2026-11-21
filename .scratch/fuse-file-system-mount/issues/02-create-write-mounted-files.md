# 02: Create and write mounted files

**What to build:** A user can create and edit regular files through the Linux mount. Exclusive creation cannot overwrite an existing entry, offset writes preserve unrelated bytes, append uses the current end, and successful writes reach the File System's existing durable Content Revision boundary. The core gains an atomic create-if-absent operation; the adapter wires it and the existing owned-handle write path through to real operating-system behavior.

**Blocked by:** 01: Mount and read a Full-residency File System

**Status:** ready-for-agent
Work: resolved

**Scope limits:** No truncate, directory mutation, rename replacement, write coalescing, Content Revision garbage collection, Passthrough writes, or claim of complete concurrent-writer POSIX behavior.

**Current evidence:** The File System already stages whole and offset writes, syncs content before metadata publication, preserves File Entry identity, and has fault-injection tests. It lacks atomic create-if-absent and all FUSE write callbacks.

- [x] A core create-if-absent operation executes in the existing serialized mutation path, requires Full Residency, returns `AlreadyExists` for any existing entry, and creates an empty regular File Entry with the existing default mode.
- [x] FUSE create and exclusive-create use the atomic core operation rather than an unprotected adapter lookup followed by a write; duplicate creation returns `EEXIST` without changing existing content or revision.
- [x] Ordinary create opens a usable owned handle, and offset writes preserve bytes before and after the written range while producing a distinct durable Content Revision with stable File Entry identity.
- [x] Append writes use current content length plus the handle's revision-checked write path; a conflicting revision fails instead of writing at an unchecked offset.
- [x] Write, flush, release, and fsync report success only after the relevant admitted File System operations complete; no second commit or persistence protocol is introduced.
- [x] Write-side errno behavior covers directories, missing entries, invalid offsets or sizes, Passthrough policy, stale revisions, cancellation, denial, unavailable prior content, and underlying I/O failures.
- [x] Real Linux mount tests cover create, exclusive create, overwrite, random-offset writes, append, flush, fsync, close, and restart persistence through ordinary OS calls.
- [x] Existing native, shared, and browser API tests establish create-if-absent behavior where applicable; existing write fault tests continue to prove that failed publication leaves the previous visible revision intact.
- [x] Pre-feature persisted data opens without migration and new creates/writes use the existing filesystem sync protocol version and appear to peers as ordinary Content Revisions.
- [x] Current crate documentation marks file creation and offset/append writing as supported only within the Linux/Full mount contract and retains the concurrency and unsupported-operation limits.
- [x] Focused core and real-mount tests, formatting, relevant feature and Clippy checks, and documentation checks pass with every qualification limit recorded.

# Comments

- 2026-11-20: Implemented serialized atomic `create_file`, native/shared/browser API exposure, FUSE ordinary/exclusive create, owned writable handles, revision-checked offset and append writes, flush/fsync/release waiting, and errno behavior. Duplicate creates preserve existing content and revisions. Native staged-copy misses now use `ContentUnavailable`, and write offsets are checked before staging.
- Core validation: `cargo test -p file-system --all-targets` passed all 77 tests. `cargo hack test --feature-powerset -p file-system --all-targets` passed all four feature combinations. Native concurrent create, duplicate content/revision preservation, shared restart, create fault cleanup, offset overflow, existing write faults, admission/cancellation/denial, and the existing versioned sync path passed. The sync test now originates content through `create_file` plus `write_at`.
- Real `/dev/fuse` and `fusermount3` were available. `cargo test -p file-system-fuse --test linux_mount -- --ignored --nocapture` passed all three tests. Coverage includes ordinary/exclusive create, duplicate `EEXIST`, writable open, offset overwrite and sparse writes, append and conflicting-append `ESTALE`, flush/fsync/close, Passthrough `EROFS`, unavailable prior content `EIO`, invalid paths, directory/missing-entry errors, stable identity, clean unmount, and restart persistence.
- Adapter validation: `cargo test -p file-system-fuse --all-targets` passed four validation tests with three real-mount tests ignored by default; its feature-powerset check passed. `cargo fmt --all -- --check`, `git diff --check`, and `just docs-check` passed.
- Focused Clippy passed with the existing `multiple_crate_versions` and `redundant_feature_names` classes plus the unrelated `ofdb-kv` `collapsible_if` lint allowed. The stricter invocation stopped at that pre-existing `ofdb-kv` lint before checking these crates.
- TypeScript `check` and Biome `lint` passed using the existing generated declarations. The new WASM build and browser runtime behavior remain unqualified: `build:wasm` is blocked by the pre-existing `ofdb-kv` missing-`io` browser errors also recorded in ticket 01. No browser runtime pass is claimed.
- Instrumented `cargo llvm-cov -p file-system -p file-system-fuse --all-targets -- --include-ignored` passed all selected tests, including real mounts. CRAP reported 12 core findings and the adapter errno mapper at 43.1; browser-only source/LCOV scope was not matched. This is recorded, not claimed as a clean CRAP result.
- Compatibility decision: no persisted field, schema migration, or filesystem sync-framing/version change was added. Existing native/shared restart tests and the existing versioned sync test pass; create/write revisions appear to peers as ordinary Content Revisions. Truncate, directory mutation, automatic authorization/Residency changes, network callbacks, and complete concurrent-writer POSIX behavior remain outside this ticket.
