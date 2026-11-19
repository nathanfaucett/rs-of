# 03: Truncate mounted files

**What to build:** A user can replace file contents through `O_TRUNC` and can shrink or extend a file through the size attribute. A revision-checked core truncate operation preserves File Entry identity and uses the same staged Content Revision commit path as other writes. Extension zero-fills new bytes, and every failure before commit preserves the previous visible revision.

**Blocked by:** 02: Create and write mounted files

**Status:** ready-for-agent

**Scope limits:** No chmod, chown, timestamp updates, atomic rename replacement, delayed write buffering, or new persisted metadata or synchronization schema.

**Current evidence:** Existing staged content supports whole writes, offset writes, append, sync, publish, and failure cleanup. It has no length-changing operation other than whole-file overwrite.

- [ ] A public revision-checked truncate operation requires Full Residency, rejects stale revisions and directory paths, preserves File Entry identity, and returns the resulting Content Revision through the existing handle contract.
- [ ] Truncation shrinks retained bytes correctly and extends with zero bytes, including truncation to zero and extension beyond the previous end.
- [ ] The internal content boundary implements length changes for native, shared, and browser backends so the core contract remains portable even though FUSE is Linux-only.
- [ ] Content staging is synced and published before metadata changes; write, sync, publish, and metadata failures preserve the previous visible Content Revision and remove abandoned staging.
- [ ] FUSE open with `O_TRUNC` and setattr with a size use the same core operation and update the relevant owned-handle revision without silently truncating through an unchecked whole-file overwrite.
- [ ] Setattr requests for unsupported ownership, mode, timestamp, or flag changes return `ENOSYS` without partially applying a requested size change.
- [ ] Real Linux mount tests cover open-time truncation, explicit shrink and extension, zero-length files, subsequent reads/writes, restart persistence, and standard editor-style replacement.
- [ ] Core tests cover current and stale revisions, all backend length implementations, sparse/zero-fill behavior, and every existing content fault checkpoint.
- [ ] Existing stores need no migration and truncate reaches updated and older peers as an ordinary new Content Revision without changing filesystem sync protocol version 1.
- [ ] Documentation states which size changes are supported and excludes all other attribute mutations and unsupported open-file transitions.
- [ ] Focused backend and real-mount tests, fault tests, formatting, relevant feature and Clippy checks, and documentation checks pass with unrun qualification recorded.

# Comments
