# 04: Mutate mounted directories

**What to build:** A user can organize a mounted Full-residency File System with `mkdir`, `unlink`, and `rmdir`. File and directory types remain distinct, rmdir rejects directories with children, and check-and-delete sequences are serialized against other mount mutations so the adapter does not silently orphan entries.

**Blocked by:** 01: Mount and read a Full-residency File System

**Status:** ready-for-agent

**Scope limits:** No rename, recursive delete, atomic rename replacement, global atomicity against independent File System callers or remote Replicas, or open-after-unlink guarantee.

**Current evidence:** The File System provides create_dir, list, and delete. create_dir is idempotent and creates parents, while delete has no POSIX type or empty-directory guard.

- [ ] FUSE mkdir returns `EEXIST` for an existing file or directory and creates one directory under an existing parent with the existing default directory mode.
- [ ] Unlink removes only regular files, rmdir removes only directories, and neither operation permits deletion of the synthetic mount root.
- [ ] Rmdir checks the existing public directory listing and returns `ENOTEMPTY` when children exist; no child metadata or content becomes orphaned by a mount operation.
- [ ] Adapter-local mutation coordination serializes mkdir, unlink, rmdir, and related check-and-act sequences without claiming atomicity against independent core callers or remote Replicas.
- [ ] Wrong-type, missing-parent, reserved-path, Passthrough-policy, stale-state, and storage errors retain their documented errno behavior.
- [ ] Directory deletion does not invent a second Tombstone or Content Revision mechanism; existing File System deletion and synchronization contracts remain authoritative.
- [ ] Real Linux mount tests cover nested mkdir, duplicate mkdir, file unlink, empty rmdir, non-empty rmdir, wrong-type calls, root deletion attempts, and subsequent directory iteration.
- [ ] Open directory handles and mount shutdown remain correct after namespace mutations, with unique handle IDs and exactly-once release behavior.
- [ ] Failed mutations preserve the prior namespace and locally retained content, and restart tests confirm the committed directory state.
- [ ] Documentation lists the supported directory operations and explicitly excludes recursive deletion, open-after-unlink guarantees, and cross-Replica atomicity.
- [ ] Focused adapter and real-mount tests, relevant core regression tests, formatting, Clippy, and documentation checks pass with qualification limits recorded.

# Comments
