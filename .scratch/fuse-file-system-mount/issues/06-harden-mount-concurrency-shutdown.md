# 06: Harden concurrent mount operations and shutdown

**What to build:** Multiple local processes can use the supported mount operations without adapter-generated revision corruption, and application shutdown reliably releases the mount and its managed copy. Concurrent readers and writers, external revision changes, pending synchronization, learned policy denial, and managed-copy retirement produce safe external behavior rather than stale writes, cross-file I/O, or leaked tasks.

**Blocked by:** 03: Truncate mounted files; 05: Rename files and directory trees without replacement

**Status:** ready-for-agent
Work: resolved

**Scope limits:** No distributed lock manager, complete POSIX concurrency or open-file model, new remote-change feed, kernel-notification optimization, or write-performance redesign.

**Current evidence:** Core operations serialize mutations and use expected Content Revisions. Existing admission, cancellation, integrity, and retirement tests cover core and transport behavior, but no FUSE handle table or mount lifecycle has been exercised.

- [x] Same-file mount mutations are coordinated sufficiently to prevent adapter-generated stale-write races across multiple open handles and operating-system processes.
- [x] Every accepted write uses the intended File Entry and current admitted Content Revision; an external conflict returns `ESTALE` or a documented safe error without silent data loss.
- [x] Reads and directory scans cannot cross to a different File Entry after delete, recreate, rename, or remote replacement; unsupported open-file transitions fail explicitly.
- [x] Kernel file and directory handle IDs remain unique, handle tables have bounded lifetimes, and flush, release, releasedir, and unmount perform exactly-once cleanup.
- [x] Concurrent offset writes, append, truncate, rename, unlink, and rmdir preserve final byte and namespace integrity according to the documented no-replace and stale-conflict rules.
- [x] Cancellation maps to `ECANCELED`, learned policy denial to `EACCES`, and retired managed copies to a clean terminal error; no callback reports success after its operation was not admitted.
- [x] Unmount with open files, active directory handles, pending synchronization, and in-flight admitted writes stops new work, finishes only operations covered by existing core admission rules, and releases remaining resources.
- [x] Learned retirement or deselection does not leave a mount thread, notifier, handle, or synchronization clone that prevents backing-root cleanup.
- [x] Real Linux mount tests exercise concurrent processes and shutdown cases, while existing fault-injection seams cover cancellation, response backpressure, publication failure, and retirement boundaries.
- [x] Documentation describes the supported local concurrency contract and explicitly excludes distributed locking and complete POSIX open-file behavior.
- [x] Focused race, fault, lifecycle, core regression, and real-mount tests pass, followed by formatting, relevant feature and Clippy checks, and documentation checks.

# Comments

- 2026-11-21: Kernel handles for one mounted path now share a revision-bound core handle. Mount mutations are serialized, external revisions still return `ESTALE`, and the last release closes the shared handle once. Unmount closes adapter state before session shutdown and falls back to `fusermount3 -uz` when open descriptors keep the normal unmount busy.
- Real Linux mounts passed: 12 existing scenarios plus seven hardening scenarios. Hardening covers multiple handles, concurrent process offset/append writes, descriptor and open truncation, external conflicts, rename/unlink/rmdir races, open file and directory shutdown, learned projected-copy eviction with `EACCES`, and pending in-memory synchronization with explicit operation policy.
- `cargo test -p file-system --all-targets` passed all 82 tests, including admission, cancellation, response-backpressure, publication-failure, and retirement drains. Both File System feature powersets passed: four core combinations and one adapter combination.
- Focused Clippy passed with only the previously recorded duplicate-version and redundant-feature-name allowances. `cargo fmt --all`, `just docs-check`, and `git diff --check` passed.
- The pending-synchronization test uses the in-memory transport for lifecycle coverage only. Authorized native Storage synchronization and remote-replacement mount behavior remain ticket 07 qualification. Open-descriptor tests emit the expected normal-unmount `EBUSY` message before the lazy fallback succeeds. No schema or sync-protocol change was made.
- Final review found and fixed a shutdown admission race. `close_all` now rejects new work with `ECANCELED` before clearing handles, while admitted operations finish. The direct regression passed, followed by all 19 adapter real mounts, both native acceptance scenarios, and focused Clippy.
