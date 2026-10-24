# 01: Move Unified public-interface tests outside its implementation

**What to build:** Unified composition and public host-lifecycle tests run as external consumers, while private implementation checks remain focused unit tests and owner-only checks live with their owners. Establish an assertion-based coverage inventory so subsequent shared-scenario migrations preserve existing guarantees.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] Classify existing tests and support fixtures by their assertions: private unit, owner-only, Unified public integration, shared Node system scenario, or deployment-specific system scenario. Record existing coverage and intended disposition without treating filenames as ownership evidence.
- [ ] Move public-interface Unified composition and host-lifecycle coverage to external integration tests using existing public interfaces; demonstrate that representative moved tests pass.
- [ ] Retain genuinely private configuration, locking, journal, and recovery checks beside their implementation. Relocate owner-only assertions where practical without introducing direct server-to-server dependencies.
- [ ] Use generated owner clients for ordinary supported HTTP operations in migrated tests. Reserve raw HTTP for intentional wire/routing assertions and use existing native protocol clients where appropriate.
- [ ] Remove replaced test modules, duplicate helpers, and private runtime hooks made obsolete by this slice. Do not widen production visibility solely to accommodate tests.
- [ ] Keep scenario fixtures minimal; do not build a new generic framework or reimplement domain behavior.
- [ ] Preserve security, recovery, and failure assertions. Record any remaining coupling or deferred move with a concrete reason and replacement plan.
- [ ] Run focused moved tests and basic affected-target checks; record commands and results. Do not run CRAP/coverage or feature-powerset testing.
