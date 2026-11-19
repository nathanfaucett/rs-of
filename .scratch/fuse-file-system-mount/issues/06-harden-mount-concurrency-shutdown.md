# 06: Harden concurrent mount operations and shutdown

**What to build:** Multiple local processes can use the supported mount operations without adapter-generated revision corruption, and application shutdown reliably releases the mount and its managed copy. Concurrent readers and writers, external revision changes, pending synchronization, learned policy denial, and managed-copy retirement produce safe external behavior rather than stale writes, cross-file I/O, or leaked tasks.

**Blocked by:** 03: Truncate mounted files; 05: Rename files and directory trees without replacement

**Status:** ready-for-agent

**Scope limits:** No distributed lock manager, complete POSIX concurrency or open-file model, new remote-change feed, kernel-notification optimization, or write-performance redesign.

**Current evidence:** Core operations serialize mutations and use expected Content Revisions. Existing admission, cancellation, integrity, and retirement tests cover core and transport behavior, but no FUSE handle table or mount lifecycle has been exercised.

- [ ] Same-file mount mutations are coordinated sufficiently to prevent adapter-generated stale-write races across multiple open handles and operating-system processes.
- [ ] Every accepted write uses the intended File Entry and current admitted Content Revision; an external conflict returns `ESTALE` or a documented safe error without silent data loss.
- [ ] Reads and directory scans cannot cross to a different File Entry after delete, recreate, rename, or remote replacement; unsupported open-file transitions fail explicitly.
- [ ] Kernel file and directory handle IDs remain unique, handle tables have bounded lifetimes, and flush, release, releasedir, and unmount perform exactly-once cleanup.
- [ ] Concurrent offset writes, append, truncate, rename, unlink, and rmdir preserve final byte and namespace integrity according to the documented no-replace and stale-conflict rules.
- [ ] Cancellation maps to `ECANCELED`, learned policy denial to `EACCES`, and retired managed copies to a clean terminal error; no callback reports success after its operation was not admitted.
- [ ] Unmount with open files, active directory handles, pending synchronization, and in-flight admitted writes stops new work, finishes only operations covered by existing core admission rules, and releases remaining resources.
- [ ] Learned retirement or deselection does not leave a mount thread, notifier, handle, or synchronization clone that prevents backing-root cleanup.
- [ ] Real Linux mount tests exercise concurrent processes and shutdown cases, while existing fault-injection seams cover cancellation, response backpressure, publication failure, and retirement boundaries.
- [ ] Documentation describes the supported local concurrency contract and explicitly excludes distributed locking and complete POSIX open-file behavior.
- [ ] Focused race, fault, lifecycle, core regression, and real-mount tests pass, followed by formatting, relevant feature and Clippy checks, and documentation checks.

# Comments
