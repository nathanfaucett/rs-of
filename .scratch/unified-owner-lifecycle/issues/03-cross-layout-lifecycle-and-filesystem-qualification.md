# 03: Qualify the lifecycle and the offline-first filesystem in both topologies

**What to build:** One cross-layout deployment suite qualifies the Unified Host and the multi-process deployment with the same scenario bodies, and extends that shared body to cover the offline-first filesystem surface. A filesystem resource is created, its content written and read, its residency changed, it is deleted, and it is read again after a retained restart — identically in both topologies — so the filesystem behavior of a composed deployment is verified rather than assumed.

**Blocked by:** 02

**Status:** resolved

- [x] The same scenario body runs against the unified topology and the multi-process topology for fresh Create, exact retry, conflicting retry, owner readiness, Management participation from supplied configuration, retained restart, and Join.
- [x] Both topologies create a filesystem resource and read it back through the generated storage client.
- [x] Both topologies write and read file content through the storage protocol.
- [x] Both topologies change residency and observe the effect.
- [x] Both topologies delete the resource and observe tombstone behavior.
- [x] Both topologies read the resource after a retained restart.
- [x] The scenarios use the generated clients and the existing storage protocol, with no raw HTTP fallback.
- [x] The scenarios fail when the deployment wiring is broken, for example when a resource is created but its content cannot be read after composition.
- [x] The filesystem crate's own in-process suite still passes unchanged.
- [x] The suite stays opt-in and process-launching, with isolated roots, bounded deadlines, and serial execution.

**Notes:** The suite stays with the deployment it launches; do not move it into the filesystem crate's workspace and do not add a cross-workspace dependency cycle.

## Comments

Claimed by ticket03 agent on `unified-owner-lifecycle-03`, isolated worktree `of-cross-layout-03`, based on integration `6b6480ab53aa9f249cacfebe138473d778aaf8bd`. Approved seams: shared production-process deployment scenarios through generated owner/storage clients and storage protocol; focused live reconciliation regression; unchanged offs suite. Complexity moderate, risk high at readiness/restart/protocol wiring, priority qualification prerequisite for ticket04.

## Answer

Implemented one owner-local production-process scenario suite in both layouts, with exact/conflicting retries, public readiness/operation evidence, Management authorization, Join/recovery/source-offline reopen, retained filesystem bytes, residency write effects and file/resource tombstones. All nine opt-in scenarios pass serially; unchanged offs workspace feature powerset passes. The target IdP Join ConnectionRefused is reproduced by delayed listener startup and fixed with bounded public-status availability polling; final Unified all-feature runtime passes both tests. See [exact commands, red/green evidence and qualification limits](../ticket03-validation.md). Broader of powerset/CRAP are blocked by concurrent sibling offs manifest duplicate keys, not claimed green.
