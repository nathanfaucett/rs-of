# 05: Rename files and directory trees without replacement

**What to build:** A user can rename a regular file or a complete directory tree through the Linux mount when the destination is absent. Existing File Entry identities and content survive the rename. Occupied destinations and unsupported rename flags fail explicitly, and the adapter never emulates replacement with a non-atomic delete followed by rename.

**Blocked by:** 04: Mutate mounted directories

**Status:** ready-for-agent

**Scope limits:** No atomic destination replacement, rename exchange, complete open-file semantics after rename or unlink, hard links, or cross-Replica atomicity.

**Current evidence:** The File System already renames files and directory trees transactionally, preserves File Entry identities, rejects occupied destinations, and prevents moving a directory beneath itself.

- [ ] FUSE rename maps source and destination parents and names to the existing relative File System paths without lossy or reserved-component aliases.
- [ ] Files, empty directories, and populated directory trees rename successfully when the destination is absent, preserving every File Entry identity and readable Content Revision.
- [ ] Occupied destinations return `EEXIST`; rename exchange and other unsupported flags return `ENOSYS`; moving a directory into its own subtree returns `EINVAL`.
- [ ] Missing sources, wrong parent types, invalid destinations, Passthrough rules, and storage failures preserve the original namespace and return documented errors.
- [ ] No code path performs destination deletion followed by rename or claims atomic replacement semantics that the core does not provide.
- [ ] Zero cache lifetimes allow subsequent lookup and directory operations to observe the new namespace without a stale adapter inode table.
- [ ] Open handles affected by an external rename, delete, or replacement cannot silently read or write a different File Entry; unsupported transitions return `ESTALE` or the documented safe error.
- [ ] Real Linux mount tests cover file and directory-tree rename, identity and content persistence, occupied destinations, invalid subtree moves, unsupported flags, and subsequent OS lookups.
- [ ] Existing core rename tests continue to pass, including transactional destination handling and identity preservation; no persisted or sync protocol shape changes are introduced.
- [ ] Documentation states no-replace behavior and the exact open-handle limitations rather than claiming complete POSIX rename semantics.
- [ ] Focused core and real-mount tests, formatting, relevant Clippy, and documentation checks pass with all unrun checks recorded.

# Comments
