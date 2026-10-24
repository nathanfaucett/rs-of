# Restore Unified Host composition boundaries

Status: resolved

Implementation: `refactor/unified-server-boundaries`. See [the responsibility audit and qualification limits](audit.md). Implementation tickets are resolved; full feature-matrix and complexity qualification remain incomplete after bounded validation attempts.

## Problem Statement

The Unified Host is intended to compose a Node's IdP, Management, Device, and Storage APIs while retaining separate domain and state ownership. Its implementation also contains provisioning and recovery workflows, durable bootstrap journals, security synchronization and admission protocol implementations, and owner-facing convenience operations. This makes a composition wrapper responsible for behavior that should be implemented behind owner interfaces.

The preceding cleanup removed some indirection and extracted some owner policy, but also relocated host workflows into Unified. That does not establish the requested ownership boundaries. Consolidating files or moving the same implementation into a new wrapper crate would not solve the problem either.

File count is not the acceptance criterion. Tests must also be evaluated by responsibility rather than their names, locations, size, or whether they exercise several APIs.

## Solution

Make Unified a composition root over existing owner APIs. It loads and combines host configuration, constructs owners, injects dependencies, mounts owner routers, registers owner-provided protocol handlers, and coordinates process startup, readiness, and shutdown.

Retain necessary cross-owner coordination only when it calls owner interfaces without implementing owner rules or inspecting their repositories. Put domain operations, validation, durable domain transitions, and protocol behavior behind their existing owning modules. Extend an owner interface only where a demonstrated existing workflow cannot otherwise be expressed; do not introduce new product features or speculative abstractions.

Preserve existing externally observable setup, serving, security, recovery, and offline-restart behavior. Remove obsolete implementations and dependencies after their callers use the correct interfaces.

## User Stories

1. As a maintainer, I want Unified to compose owner APIs, so that domain behavior has one authoritative implementation.
2. As a maintainer, I want ownership determined by responsibility rather than file count, so that cleanup improves architecture rather than appearance.
3. As a maintainer, I want host configuration to combine owner settings without duplicating owner validation, so that configuration rules remain local to their owners.
4. As a maintainer, I want owners constructed once and their handles reused, so that setup and serving operate on the same owner instances.
5. As a maintainer, I want routers composed without reimplementing their handlers, so that bundled and independent deployments retain the same owner contracts.
6. As a maintainer, I want owner-provided protocol handlers registered by the host, so that protocol implementation does not accumulate in the composition root.
7. As a maintainer, I want provisioning operations expressed through owner interfaces, so that Unified does not create or interpret domain records.
8. As a maintainer, I want Join and admission rules enforced by their owners, so that transport composition cannot grant domain authority.
9. As a maintainer, I want recovery and retry invariants enforced where their durable transitions are owned, so that the wrapper does not implement a second state machine.
10. As a maintainer, I want cross-owner coordination retained when genuinely necessary, so that enforcing ownership does not prevent process composition.
11. As a maintainer, I want coordination to avoid direct repository access, so that hosting contexts together does not merge their state ownership.
12. As a maintainer, I want local and HTTP integrations to use the same owner behavior, so that deployments do not acquire divergent security rules.
13. As an operator, I want existing Create, Join, retry, cancellation, recovery, and Reset Device behavior preserved, so that this refactor does not alter operational contracts.
14. As a User, I want credentials and private signer material to remain local and protected, so that architectural cleanup does not weaken secret separation.
15. As a Node operator, I want completed state to reopen offline, so that composition does not introduce a central-service dependency.
16. As a User, I want learned revocations and restrictions checked at subsequent execution points, so that an open connection cannot bypass current authorization.
17. As a maintainer, I want shutdown to release composed owners, listeners, and tasks, so that extraction does not leak runtime resources.
18. As a maintainer, I want owner behavior tested through owner interfaces, so that tests enforce contracts rather than private implementation structure.
19. As a maintainer, I want legitimate cross-owner integration tests retained, so that ownership cleanup does not discard unique behavioral coverage.
20. As a maintainer, I want duplicate tests and obsolete fixtures removed, so that moving responsibilities does not preserve unnecessary maintenance work.
21. As a maintainer, I want dependency direction preserved, so that extracting behavior does not create server-to-server dependency cycles.
22. As a maintainer, I want design documentation to match actual responsibility, so that future work does not rebuild the same architectural problem.

## Implementation Decisions

- Unified owns host configuration composition, concrete owner construction and dependency injection, router and endpoint composition, handler registration, and process lifecycle.
- Unified must not own domain policy, authorization decisions, domain persistence schemas, provisioning or admission state machines, or security replication protocol implementations.
- Necessary host-level owner-call ordering may remain. Calling two owners is not itself a violation. Interpreting domain records, duplicating owner validation, or deciding domain transitions is a violation.
- Distinguish process-level orchestration state from domain workflow state before relocating journals. A nonsecret host reference or process lock is not automatically misplaced; a journal that determines provisioning or admission transitions is not justified merely by being called host state.
- Audit bootstrap configuration, credential bootstrap, local provisioning, local Join, setup hosting/runtime, security synchronization/bootstrap, endpoint discovery, bootstrap verification, and runtime convenience methods against these rules. Do not treat any current module as approved wholesale.
- Assign extracted behavior according to existing ownership: IdP owns identity, credentials, OAuth Clients and signing trust; Management owns roles, permissions and assignments; Device owns Device lifecycle, endpoint binding, admission and replication policy; Storage owns resource lifecycle and authorization.
- Reusable persistence and synchronization mechanics remain with their existing persistence owners; generic networking mechanics remain with their networking owner. Installation-specific policy must not be pushed into generic infrastructure.
- Prefer existing concrete owner interfaces. Add only the minimum owner operation needed to encapsulate existing behavior. Do not introduce a generic host callback framework, a new catch-all orchestration crate, or a parallel implementation.
- Do not move an entire cross-owner implementation into one server if that creates prohibited direct server-to-server dependencies. Separate owner behavior from composition at existing seams instead.
- Preserve route prefixes, request/response contracts, CLI behavior, admission semantics, retained journal formats, exact binding checks, and failure behavior. No compatibility framework or migration mechanism is authorized by this refactor.
- Preserve one shared endpoint/router, owner-handle reuse, existing-only completed reopen, fail-closed malformed-state handling, cancellation/finalization ordering, secret separation, and execution-time authorization checks.
- Keep local and HTTP owner paths aligned; composition must not bypass owner authorization.
- Delete superseded implementations, exports, fixtures, and unused dependencies after callers are updated. Do not retain old implementations behind adapters.
- Keep crate roots limited to declarations and re-exports. File layout follows responsibility; no target file count or percentage reduction applies.
- Update stable design and API documentation where necessary. Existing design prose explicitly assigns setup journals and protocol-specific adapters to Unified; reconcile that prose with the narrower composition responsibility rather than silently leaving contradictory guidance.

## Testing Decisions

- Proposed seams: existing public owner interfaces for owner behavior, plus Unified's existing public construction/router/serving interfaces for composition and lifecycle. No new test-only seam or generalized dependency abstraction is planned.
- Ask the maintainer to confirm these seams when presenting the published spec; they are proposed, not represented as separately confirmed.
- Good tests assert observable contracts and failures, not private module layout, helper invocation counts, or the number of files remaining.
- Owner-level tests cover extracted domain validation, authorization, durable transitions, synchronization/admission behavior, and retry/recovery invariants through the owning interface.
- Unified composition tests cover owner construction and handle reuse, route prefix mounting, injected dependencies, shared endpoint and handler wiring, readiness, listener integration, and clean shutdown.
- Retain cross-owner integration tests where they prove unique interactions. Exercising Create, Join, permissions, or recovery does not automatically make a test inappropriate for Unified.
- Review existing setup, bootstrap, security, permission, cleanup, residency, full-node, transport, and owner-composition suites by their assertions. Move owner-only coverage, retain composition coverage, split mixed tests only where necessary, and delete demonstrably duplicate coverage.
- Prefer existing composition integration tests and existing owner security-policy and copy-retirement tests as prior art. Do not build a replacement testing framework.
- Characterize failures before attributing them to this work. Earlier broad Unified validation timed out and reported unresolved setup, permission-inventory, route-expectation, and generated-client qualification failures; they were not established as baseline failures.
- Run focused changed-owner tests and composition tests first. Serialize credential-store-sensitive scenarios where global test state requires it.
- Final validation includes affected all-target/all-feature checks, formatting, Clippy, dependency-direction checks, the required feature-powerset tests, and the project complexity check. Avoid concurrent Cargo-heavy validation that introduces lock contention. Report failures and timeouts explicitly; do not claim incomplete runs passed.

## Out of Scope

- A file-count target, a 90% reduction requirement, or cosmetic merging of implementations.
- New endpoints, features, deployment modes, permission models, or user workflows.
- Blanket relocation or deletion of tests based on filenames or cross-owner coverage.
- A new wrapper/core crate, generic orchestration framework, speculative interfaces, or compatibility layer.
- Weakening validation, authorization, crash safety, cancellation, TLS trust, or recovery guarantees to make extraction easier.
- Changing domain ownership, introducing privileged Nodes, requiring live central authorization, or rewriting neighboring infrastructure projects.
- Unrelated test or lint cleanup beyond what is necessary to verify the changed contracts.

## Further Notes

This spec captures the latest direction: focus on ownership boundaries and preserve legitimate composition. Earlier suggestions to delete entire test categories or achieve a fixed file reduction are superseded.

The previous implementation remains an input to review, not evidence that the requested architectural work is complete. Some owner-policy and cleanup deduplication may already be correctly placed; retain useful work without preserving misplaced host implementations.

Acceptance requires a responsibility audit of retained Unified behavior, owner-facing tests for extracted behavior, preserved composition coverage, removal of superseded implementations, and documentation that accurately describes the resulting boundaries.
