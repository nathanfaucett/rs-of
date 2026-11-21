# 04: Mutate mounted directories

**What to build:** A user can organize a mounted Full-residency File System with `mkdir`, `unlink`, and `rmdir`. File and directory types remain distinct, rmdir rejects directories with children, and check-and-delete sequences are serialized against other mount mutations so the adapter does not silently orphan entries.

**Blocked by:** 01: Mount and read a Full-residency File System

**Status:** ready-for-agent
Work: resolved

**Scope limits:** No rename, recursive delete, atomic rename replacement, global atomicity against independent File System callers or remote Replicas, or open-after-unlink guarantee.

**Current evidence:** The File System provides create_dir, list, and delete. create_dir is idempotent and creates parents, while delete has no POSIX type or empty-directory guard.

- [x] FUSE mkdir returns `EEXIST` for an existing file or directory and creates one directory under an existing parent with the existing default directory mode.
- [x] Unlink removes only regular files, rmdir removes only directories, and neither operation permits deletion of the synthetic mount root.
- [x] Rmdir checks the existing public directory listing and returns `ENOTEMPTY` when children exist; no child metadata or content becomes orphaned by a mount operation.
- [x] Adapter-local mutation coordination serializes mkdir, unlink, rmdir, and related check-and-act sequences without claiming atomicity against independent core callers or remote Replicas.
- [x] Wrong-type, missing-parent, reserved-path, Passthrough-policy, stale-state, and storage errors retain their documented errno behavior.
- [x] Directory deletion does not invent a second Tombstone or Content Revision mechanism; existing File System deletion and synchronization contracts remain authoritative.
- [x] Real Linux mount tests cover nested mkdir, duplicate mkdir, file unlink, empty rmdir, non-empty rmdir, wrong-type calls, root deletion attempts, and subsequent directory iteration.
- [x] Open directory handles and mount shutdown remain correct after namespace mutations, with unique handle IDs and exactly-once release behavior.
- [x] Failed mutations preserve the prior namespace and locally retained content, and restart tests confirm the committed directory state.
- [x] Documentation lists the supported directory operations and explicitly excludes recursive deletion, open-after-unlink guarantees, and cross-Replica atomicity.
- [x] Focused adapter and real-mount tests, relevant core regression tests, formatting, Clippy, and documentation checks pass with qualification limits recorded.

# Comments

- 2026-11-20: Implemented FUSE mkdir, unlink, and rmdir over the existing public File System contracts. Create, mkdir, unlink, and rmdir share one adapter-local mutation mutex; rmdir holds it across the public-list emptiness check and existing delete call. Mkdir requires an existing directory parent and returns `EEXIST` for either existing type. Unlink/rmdir enforce distinct types, path errors, Passthrough policy, and root protection without recursive deletion or a second deletion mechanism.
- Real `/dev/fuse` and `fusermount3` were available. `cargo test -p file-system-fuse --test linux_mount -- --ignored --nocapture` passed all seven tests. Coverage includes nested/duplicate mkdir, default modes, missing/wrong/stale parents, file unlink, empty/non-empty/wrong-type/root rmdir, root unlink, exact path/policy/unavailable-content errors, failed-mutation preservation, stable held directory snapshots and offsets, independent file/directory handles, release/unmount, subsequent iteration, unsupported rename, and deleted-state persistence across a real remount.
- `cargo test -p file-system-fuse --all-targets` passed four validation tests with the seven real mounts ignored by default; their separate explicit pass is above. `cargo test -p file-system --all-targets` passed all 77 tests, including existing deletion, tombstone synchronization, restart, residency, and fault coverage. `cargo hack test --feature-powerset -p file-system --all-targets` passed all four feature combinations; the adapter powerset passed its single combination.
- `cargo fmt --all -- --check`, `git diff --check`, adapter Rustdoc, and `just docs-check` passed. Focused all-target/all-feature Clippy passed with the existing `multiple_crate_versions`, `redundant_feature_names`, and unrelated `ofdb-kv` `collapsible_if` classes allowed.
- Instrumented `cargo llvm-cov -p file-system -p file-system-fuse --all-targets -- --include-ignored` passed all selected tests, including all seven real mounts. Focused CRAP reported 12 existing core findings and the existing adapter errno mapper at 43.1; mkdir, unlink, and rmdir remained below threshold. Browser-only source/LCOV scope was not matched. This is not a clean CRAP or browser-runtime claim.
- Compatibility decision: no core source, persisted field, schema migration, Tombstone format, Content Revision mechanism, or filesystem sync-protocol change was added. Existing core deletion and versioned synchronization contracts remain authoritative. Coordination excludes independent core callers and remote Replicas. Rename, truncate, recursive deletion, open-after-unlink guarantees, active-handle shutdown hardening, and cross-Replica atomicity remain outside this ticket. Work remains in progress.
