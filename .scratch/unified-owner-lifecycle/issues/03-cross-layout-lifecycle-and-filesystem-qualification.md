# 03: Qualify the lifecycle and the offline-first filesystem in both topologies

**What to build:** One cross-layout deployment suite qualifies the Unified Host and the multi-process deployment with the same scenario bodies, and extends that shared body to cover the offline-first filesystem surface. A filesystem resource is created, its content written and read, its residency changed, it is deleted, and it is read again after a retained restart — identically in both topologies — so the filesystem behavior of a composed deployment is verified rather than assumed.

**Blocked by:** 02

**Status:** ready-for-agent

- [ ] The same scenario body runs against the unified topology and the multi-process topology for fresh Create, exact retry, conflicting retry, owner readiness, Management participation from supplied configuration, retained restart, and Join.
- [ ] Both topologies create a filesystem resource and read it back through the generated storage client.
- [ ] Both topologies write and read file content through the storage protocol.
- [ ] Both topologies change residency and observe the effect.
- [ ] Both topologies delete the resource and observe tombstone behavior.
- [ ] Both topologies read the resource after a retained restart.
- [ ] The scenarios use the generated clients and the existing storage protocol, with no raw HTTP fallback.
- [ ] The scenarios fail when the deployment wiring is broken, for example when a resource is created but its content cannot be read after composition.
- [ ] The filesystem crate's own in-process suite still passes unchanged.
- [ ] The suite stays opt-in and process-launching, with isolated roots, bounded deadlines, and serial execution.

**Notes:** The suite stays with the deployment it launches; do not move it into the filesystem crate's workspace and do not add a cross-workspace dependency cycle.
