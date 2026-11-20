# 05: Rename files and directory trees without replacement

**What to build:** A user can rename a regular file or a complete directory tree through the Linux mount when the destination is absent. Existing File Entry identities and content survive the rename. Occupied destinations and unsupported rename flags fail explicitly, and the adapter never emulates replacement with a non-atomic delete followed by rename.

**Blocked by:** 04: Mutate mounted directories

**Status:** ready-for-agent
**Work:** in progress

**Scope limits:** No atomic destination replacement, rename exchange, complete open-file semantics after rename or unlink, hard links, or cross-Replica atomicity.

**Current evidence:** The File System already renames files and directory trees transactionally, preserves File Entry identities, rejects occupied destinations, and prevents moving a directory beneath itself.

- [x] FUSE rename maps source and destination parents and names to the existing relative File System paths without lossy or reserved-component aliases.
- [x] Files, empty directories, and populated directory trees rename successfully when the destination is absent, preserving every File Entry identity and readable Content Revision.
- [x] Occupied destinations return `EEXIST`; rename exchange and other unsupported flags return `ENOSYS`; moving a directory into its own subtree returns `EINVAL`.
- [x] Missing sources, wrong parent types, invalid destinations, Passthrough rules, and storage failures preserve the original namespace and return documented errors.
- [x] No code path performs destination deletion followed by rename or claims atomic replacement semantics that the core does not provide.
- [x] Zero cache lifetimes allow subsequent lookup and directory operations to observe the new namespace without a stale adapter inode table.
- [x] Open handles affected by an external rename, delete, or replacement cannot silently read or write a different File Entry; unsupported transitions return `ESTALE` or the documented safe error.
- [x] Real Linux mount tests cover file and directory-tree rename, identity and content persistence, occupied destinations, invalid subtree moves, unsupported flags, and subsequent OS lookups.
- [x] Existing core rename tests continue to pass, including transactional destination handling and identity preservation; no persisted or sync protocol shape changes are introduced.
- [x] Documentation states no-replace behavior and the exact open-handle limitations rather than claiming complete POSIX rename semantics.
- [x] Focused core and real-mount tests, formatting, relevant Clippy, and documentation checks pass with all unrun checks recorded.

# Comments

- 2026-11-20: Implemented FUSE rename and rename2 over the existing UTF-8 parent/name conversion and transactional `FileSystem::rename`. Ordinary rename and `RENAME_NOREPLACE` preserve File Entry identities and readable content without destination deletion. Both parent directories are checked before core rename, and rename shares adapter-local mutation coordination. No core production code, persisted format, schema, or sync protocol changed.
- Real `/dev/fuse` and `fusermount3` were available. `cargo test -p file-system-fuse --test linux_mount -- --ignored --nocapture --test-threads=1` passed all 12 tests. New coverage includes exact Unicode names, files, empty directories, populated trees, restart/remount, stable identities/content, same-type occupied destinations, cross-type errors, subtree moves, missing/wrong parents, invalid/non-UTF-8 paths, Passthrough source/destination rules, unavailable content, zero-cache lookup/iteration, and open-handle safety after mount or external rename/delete/recreate/replacement. Missing old paths return `ENOENT`; recreated or replaced paths return `ESTALE` without changing the new entry.
- `RENAME_EXCHANGE`, `RENAME_WHITEOUT`, combined flags, and an unknown flag return `ENOSYS` in the direct adapter test without changing either entry. Linux FUSE converts a rename2 `ENOSYS` reply to `EINVAL` and disables later rename2 calls on that mount. The real-mount test asserts and the documentation records this kernel behavior; it does not claim that the OS returns `ENOSYS`. Successful flagged calls complete before unsupported-flag calls. Hard links, exchange, replacement, and complete open-file POSIX semantics remain unsupported.
- `cargo test -p file-system --all-targets` passed all 82 tests, including existing core rename identity, occupied-destination, subtree, residency, restart, and versioned sync tests plus new sync/publish fault preservation and cleanup coverage. `cargo test -p file-system-fuse --all-targets` passed six non-mount tests with the 12 real mounts ignored by default; their separate explicit pass is above. `cargo hack test --feature-powerset -p file-system --all-targets` passed all four feature combinations with 82 tests each, and the adapter powerset passed its single combination.
- `cargo fmt --all -- --check`, `git diff --check`, adapter Rustdoc with warnings denied, and `just docs-check` passed. Focused all-target/all-feature Clippy passed after one ticket-local parameter-name fix, using the previously recorded `multiple_crate_versions`, `redundant_feature_names`, and unrelated `ofdb-kv` `collapsible_if` exceptions.
- Instrumented `cargo llvm-cov -p file-system -p file-system-fuse --all-targets -- --include-ignored --test-threads=1` passed every selected test, including all 12 real mounts. Per-worktree CRAP reported 12 existing core findings and the existing adapter errno mapper at 34.4; new rename and rename2 functions scored 7.0 and 2.0. Browser-only source/LCOV scope was not matched. An initial package-scoped CRAP command analyzed the wrong checkout and was discarded; explicit per-crate worktree reports are authoritative. This is not a clean CRAP or browser-runtime claim.
- Compatibility decision: rename uses the existing persisted File Entry and Content Revision formats and sync protocol version 1. No destination replacement, destination deletion, schema migration, or wire-shape change was added. Complete workspace, browser/WASM, and multi-Device runtime checks were not run because this ticket does not change those contracts. Work remains in progress.
