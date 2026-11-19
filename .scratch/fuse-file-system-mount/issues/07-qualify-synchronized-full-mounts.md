# 07: Qualify synchronized Full Replicas and publish support limits

**What to build:** A mounted Full Replica participates in the existing authorized native Storage synchronization flow without becoming a second sync owner. After remote metadata and its required content are locally available, subsequent operating-system operations observe the new state with the mount's zero-cache policy. Final documentation names the tested Linux/Full workflow, compatibility decision, and every remaining qualification limit.

**Blocked by:** 06: Harden concurrent mount operations and shutdown

**Status:** ready-for-agent

**Scope limits:** No Passthrough fetching, automatic Full-content materialization, live kernel notifications, new sync protocol, claim of immediate global revocation, or blanket completion of every M4 filesystem scenario.

**Current evidence:** Native Storage tests exercise filesystem metadata and remote file sessions, while core tests cover explicit authorized content fetch and convergence. Filesystem synchronization is not qualified by SQL tests or mesh connectivity, and no real mounted two-Replica workflow exists.

- [ ] Acceptance uses the existing native Storage filesystem transport integration seam with explicit per-peer Resource and operation policy; no default permissive OperationControl is used.
- [ ] Two Full Replicas mounted through the public adapter perform supported local edits, synchronize through the authorized native path, and converge for the covered scenario without adapter-specific persistence or replay logic.
- [ ] After remote metadata arrives, the test makes required content locally available through the existing authorized fetch path and proves subsequent OS lookup, getattr, open, read, and readdir observe it under zero cache lifetimes.
- [ ] Missing remote content returns the documented error instead of an empty file or stale successful result; local operations on already retained revisions continue without a network.
- [ ] Offline edits on each Replica survive restart and converge through the existing protocol for the tested create, write, truncate, delete, and no-replace rename cases.
- [ ] Learned revocation, deselection, Resource deletion, and managed-copy retirement block later covered operations according to current policy; tests do not claim immediate global revocation or remote erasure.
- [ ] Mount shutdown during an active native sync session leaves no adapter task or handle that prevents session termination and managed-copy cleanup.
- [ ] A pre-feature persisted Replica opens without migration, and new create/write/truncate operations synchronize through filesystem sync protocol version 1 as ordinary Content Revisions.
- [ ] In-memory transport tests remain focused fault coverage only; SQL, browser, or mesh success is not presented as filesystem-mount qualification.
- [ ] Current crate and repository support documentation lists Linux, Full Residency, supported operations, error behavior, concurrency limits, unsupported POSIX features, and all unrun platform or multi-Device checks.
- [ ] Focused and real native synchronization/mount tests pass, followed by the appropriate feature-powerset, Clippy, CRAP, and documentation checks; any unavailable kernel or backend qualification is recorded rather than claimed.

# Comments
