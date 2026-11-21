# 07: Qualify synchronized Full Replicas and publish support limits

**What to build:** A mounted Full Replica participates in the existing authorized native Storage synchronization flow without becoming a second sync owner. After remote metadata and its required content are locally available, subsequent operating-system operations observe the new state with the mount's zero-cache policy. Final documentation names the tested Linux/Full workflow, compatibility decision, and every remaining qualification limit.

**Blocked by:** 06: Harden concurrent mount operations and shutdown

**Status:** ready-for-agent
Work: resolved

**Scope limits:** No Passthrough fetching, automatic Full-content materialization, live kernel notifications, new sync protocol, claim of immediate global revocation, or blanket completion of every M4 filesystem scenario.

**Current evidence:** Native Storage tests exercise filesystem metadata and remote file sessions, while core tests cover explicit authorized content fetch and convergence. Filesystem synchronization is not qualified by SQL tests or mesh connectivity, and no real mounted two-Replica workflow exists.

- [x] Acceptance uses the existing native Storage filesystem transport integration seam with explicit per-peer Resource and operation policy; no default permissive OperationControl is used.
- [x] Two Full Replicas mounted through the public adapter perform supported local edits, synchronize through the authorized native path, and converge for the covered scenario without adapter-specific persistence or replay logic.
- [x] After remote metadata arrives, the test makes required content locally available through the existing authorized fetch path and proves subsequent OS lookup, getattr, open, read, and readdir observe it under zero cache lifetimes.
- [x] Missing remote content returns the documented error instead of an empty file or stale successful result; local operations on already retained revisions continue without a network.
- [x] Offline edits on each Replica survive restart and converge through the existing protocol for the tested create, write, truncate, delete, and no-replace rename cases.
- [x] Learned revocation, deselection, Resource deletion, and managed-copy retirement block later covered operations according to current policy; tests do not claim immediate global revocation or remote erasure.
- [x] Mount shutdown during an active native sync session leaves no adapter task or handle that prevents session termination and managed-copy cleanup.
- [x] A pre-feature persisted Replica opens without migration, and new create/write/truncate operations synchronize through filesystem sync protocol version 1 as ordinary Content Revisions.
- [x] In-memory transport tests remain focused fault coverage only; SQL, browser, or mesh success is not presented as filesystem-mount qualification.
- [x] Current crate and repository support documentation lists Linux, Full Residency, supported operations, error behavior, concurrency limits, unsupported POSIX features, and all unrun platform or multi-Device checks.
- [x] Focused and real native synchronization/mount tests pass, followed by the appropriate feature-powerset, Clippy, CRAP, and documentation checks; any unavailable kernel or backend qualification is recorded rather than claimed.

# Comments

- 2026-11-21: Added two real Linux/native Storage acceptance scenarios. Two mounted Full Replicas converged mounted create/write/truncate/delete/no-replace-rename edits through authorized Iroh transport with explicit per-peer Resource and operation policy. The test proved missing-content `EIO`, authorized fetch, zero-cache lookup/attributes/open/read/readdir, offline retained reads, actual Storage database reopen, stable identities, and unmount during active native synchronization.
- The policy scenario proved an authorized remote open before denial, learned deselection and Device revocation blocking later remote operations, retained local reads without immediate erasure, deleted-Resource reopen denial, and managed-copy retirement returning `EACCES` through the mount. Cleanup and endpoint shutdown completed without leftover mounts.
- `cargo test -p storage-server real_mount -- --ignored --nocapture --test-threads=1` passed both native mount scenarios. The existing native filesystem execution suite passed all six non-ignored tests. Adapter suites passed all 12 mount and seven hardening scenarios serially. Core passed all 82 tests; all four core and one adapter feature-powerset combinations passed.
- Focused `file-system` and `file-system-fuse` Clippy passed with only the previously recorded duplicate-version and redundant-feature-name allowances. Strict storage-server test Clippy is not clean: it stops on about 1,000 pre-existing test-lint errors across unrelated modules. The new test compile warning was fixed; no clean storage-server Clippy result is claimed.
- Instrumented core/adapter tests passed, including all 19 adapter mounts. CRAP reported zero adapter findings and 12 existing core findings, worst `Error::fmt` at 363.1. Browser-only source/LCOV scope was unmatched; this is not a clean core CRAP result.
- `cargo fmt --all`, `just docs-check`, and `git diff --check` passed. Compatibility remains unchanged: legacy JSON decoding and native/shared restart tests pass, reopened Replicas need no migration, and create/write/truncate synchronize as ordinary Content Revisions through filesystem sync protocol version 1.
- Qualification is limited to the documented Linux/Full scenarios. Browser/WASM runtime remains unavailable because of the pre-existing `ofdb-kv` missing-`io` build errors. Windows, macOS, Passthrough mounts, automatic materialization, live notifications, distributed locking, immediate global revocation, and blanket M4/multi-Device completion remain unqualified. In-memory transport supplies only focused lifecycle/fault evidence.
