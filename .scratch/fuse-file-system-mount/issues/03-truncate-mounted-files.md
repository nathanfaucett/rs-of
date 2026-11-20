# 03: Truncate mounted files

**What to build:** A user can replace file contents through `O_TRUNC` and can shrink or extend a file through the size attribute. A revision-checked core truncate operation preserves File Entry identity and uses the same staged Content Revision commit path as other writes. Extension zero-fills new bytes, and every failure before commit preserves the previous visible revision.

**Blocked by:** 02: Create and write mounted files

**Status:** ready-for-agent
**Work:** in progress

**Scope limits:** No chmod, chown, timestamp updates, atomic rename replacement, delayed write buffering, or new persisted metadata or synchronization schema.

**Current evidence:** Existing staged content supports whole writes, offset writes, append, sync, publish, and failure cleanup. It has no length-changing operation other than whole-file overwrite.

- [x] A public revision-checked truncate operation requires Full Residency, rejects stale revisions and directory paths, preserves File Entry identity, and returns the resulting Content Revision through the existing handle contract.
- [x] Truncation shrinks retained bytes correctly and extends with zero bytes, including truncation to zero and extension beyond the previous end.
- [x] The internal content boundary implements length changes for native, shared, and browser backends so the core contract remains portable even though FUSE is Linux-only.
- [x] Content staging is synced and published before metadata changes; write, sync, publish, and metadata failures preserve the previous visible Content Revision and remove abandoned staging.
- [x] FUSE open with `O_TRUNC` and setattr with a size use the same core operation and update the relevant owned-handle revision without silently truncating through an unchecked whole-file overwrite.
- [x] Setattr requests for unsupported ownership, mode, timestamp, or flag changes return `ENOSYS` without partially applying a requested size change.
- [x] Real Linux mount tests cover open-time truncation, explicit shrink and extension, zero-length files, subsequent reads/writes, restart persistence, and standard editor-style replacement.
- [x] Core tests cover current and stale revisions, all backend length implementations, sparse/zero-fill behavior, and every existing content fault checkpoint.
- [x] Existing stores need no migration and truncate reaches updated and older peers as an ordinary new Content Revision without changing filesystem sync protocol version 1.
- [x] Documentation states which size changes are supported and excludes all other attribute mutations and unsupported open-file transitions.
- [x] Focused backend and real-mount tests, fault tests, formatting, relevant feature and Clippy checks, and documentation checks pass with unrun qualification recorded.

# Comments


- 2026-11-20: Implemented public revision-checked `FileSystem::truncate` and owned-handle truncation with resulting-revision updates. Native, shared, and browser staged stores implement shrink and zero-fill extension. Content sync, publication, and metadata retain the existing order and cleanup behavior. FUSE open/create `O_TRUNC` and path/descriptor size setattr use the same operation; unsupported attribute components return `ENOSYS` before size mutation. FUSE lock-owner context is ignored because it is not an attribute mutation.
- Core validation: `cargo test -p file-system --all-targets` passed all 81 tests. `cargo hack test --feature-powerset -p file-system --all-targets` passed all four feature combinations with 81 tests each. Coverage includes current/stale revisions, Full/Passthrough enforcement, stable identity, shrink/extension/zero length, subsequent owned-handle I/O, native/shared restart, every write/sync/publish/metadata fault checkpoint, and explicit version-1 metadata sync plus content fetch of a truncate revision.
- Real `/dev/fuse` and `fusermount3` were available. `cargo test -p file-system-fuse --test linux_mount -- --ignored --nocapture --test-threads=1` passed all eight tests. The new scenario covers open/create truncation, path/descriptor shrink and extension, zero length and zero-fill, subsequent reads/writes, stable identity, editor-style `fs::write` replacement, unsupported mode/timestamp preservation, new-file creation, restart, and remount. An initial parallel full-mount run stopped at the obsolete `fs::write` `ENOSYS` assertion and then timed out with busy sessions; after updating that assertion and cleaning the sessions, the complete serial suite passed. Two killed harness processes from the timed-out run remain in kernel `request_wait_answer`; no matching mounts remain.
- Adapter validation: `cargo test -p file-system-fuse --all-targets` passed five non-mount tests with eight real mounts ignored by default. The direct adapter test proves mode, UID, GID, atime, mtime, and ctime combined with size all return `ENOSYS` without changing bytes or revision, and that lock-owner context does not block size changes. `cargo hack test --feature-powerset -p file-system-fuse --all-targets` passed its single combination.
- `cargo fmt --all -- --check`, `git diff --check`, focused all-target/all-feature Clippy, Rustdoc with warnings denied, and `just docs-check` passed. Clippy used the previously recorded `multiple_crate_versions`, `redundant_feature_names`, and unrelated `ofdb-kv` `collapsible_if` exceptions.
- Instrumented `cargo llvm-cov -p file-system -p file-system-fuse --all-targets -- --include-ignored --test-threads=1` passed every selected test, including all eight real mounts. Focused CRAP reported 12 existing core findings and the existing adapter errno mapper at 43.1; truncate and setattr stayed below threshold. Browser-only source/LCOV scope was not matched. This is not a clean CRAP result.
- The direct WASM check remains unavailable: `cargo check --manifest-path fs/file-system-wasm/Cargo.toml --target wasm32-unknown-unknown --features browser --locked` stops in the pre-existing `ofdb-kv` `database.rs` missing-`io` errors recorded in tickets 01 and 02. No WASM build or browser runtime pass is claimed. Native and shared backend behavior passed; the browser length implementation is delivered but unqualified.
- Compatibility decision: no persisted field, schema migration, or filesystem sync-framing/version change was added. Existing native/shared restart tests and the version-1 sync/fetch path pass; truncate reaches peers as an ordinary new Content Revision. Chmod, chown, timestamps, platform attribute flags, rename, later hardening, and complete concurrent-writer POSIX behavior remain outside this ticket. Work remains in progress.
