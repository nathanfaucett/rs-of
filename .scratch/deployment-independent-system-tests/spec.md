# Deployment-independent Node system tests

Status: ready-for-agent

## Problem Statement

Most Unified Host tests are compiled as internal unit-test modules even when they exercise public integration or full-Node behavior. Fixtures reach into runtime handles, databases, and test-only accessors, coupling system scenarios to the Unified implementation. This obscures which tests verify host wiring, which verify owner behavior, and which should qualify a Node regardless of deployment layout.

A passing Unified scenario does not establish that the same behavior works when IdP, Management, Device, and Storage run as separate services. Existing targeted remote-process fixtures are useful evidence but are not substitutes for complete standalone Node lifecycle scenarios. Handwritten HTTP operations also duplicate published contracts and can miss generated-client drift.

## Solution

Organize tests by the behavior and public interface they exercise, not by their historical filename or where their fixture was first implemented.

Keep genuine private unit tests with their implementation. Move public-interface Unified composition and host lifecycle tests into external integration tests. Extract deployment-independent full-Node scenarios into a shared system-test location outside the Unified implementation and execute them against both Unified and independent-service deployments.

Use generated owner HTTP clients for supported operations and existing protocol clients for non-HTTP operations. Share scenario assertions; vary only the concrete deployment launch, documented setup sequence, addressing, and lifecycle control. Preserve deployment-specific tests where the observable contracts genuinely differ.

## User Stories

1. As a maintainer, I want public integration tests separated from private unit tests, so that test placement communicates what contract is being verified.
2. As a Unified Host maintainer, I want composition tests to consume public interfaces, so that they do not require runtime internals to be exposed.
3. As an owner maintainer, I want owner-specific validation tests in the owning module or crate, so that failures and fixes remain local to that owner.
4. As a system maintainer, I want one set of full-Node behavioral assertions, so that Unified and independent deployments cannot silently diverge.
5. As an operator, I want a fresh Installation established through supported Create operations in both layouts, so that qualification represents actual setup.
6. As an operator, I want readiness checked through public contracts, so that an accepted request or open listener is not mistaken for completed setup.
7. As an Initial Administrator, I want ordinary login and authorized operations tested after Create, so that ready Nodes are demonstrably usable.
8. As an operator, I want independent setup to use each worker's own admission capability, so that test reuse does not erase host-specific authorization.
9. As an operator, I want correlated Installation and operation evidence checked across independent owners, so that unrelated ready workers cannot pass as one Node.
10. As a User, I want authorized Join through a Trusted Peer tested in both layouts, so that membership does not depend on Unified internals.
11. As an administrator, I want pending Join approval and rejection verified through Device clients, so that tests follow current ownership.
12. As an operator, I want completed Nodes reopened from retained state, so that restart coverage does not replay fresh setup.
13. As an operator, I want a completed joining Node to reopen without its source peer, so that offline independence remains verified.
14. As an operator, I want applicable interrupted setup and exact retry cases preserved, so that extracting tests does not discard recovery guarantees.
15. As a User, I want permissions and resource access checked through public operations, so that tests prove behavior rather than expected database rows.
16. As a User, I want applicable synchronization and learned-revocation scenarios shared across layouts, so that transport trust is not mistaken for continuing authorization.
17. As an operator, I want separate-process startup, shutdown, and restart exercised, so that in-process composition cannot hide deployment failures.
18. As a client maintainer, I want supported HTTP operations exercised through generated clients, so that contract and client drift becomes visible.
19. As a security maintainer, I want malformed-wire and exact transport assertions retained where necessary, so that generated-client usage does not remove negative coverage.
20. As a protocol maintainer, I want non-HTTP scenarios to use the appropriate existing protocol clients, so that HTTP client reuse does not bypass native behavior.
21. As a maintainer, I want duplicate scenarios and unnecessary private test hooks deleted, so that the reorganization reduces coupling rather than adding another suite.
22. As a developer, I want bounded diagnostics and reliable teardown, so that failures are actionable without secret disclosure, huge buffers, or abandoned processes.
23. As a reviewer, I want an explicit coverage map and honest validation results, so that moved, shared, deployment-specific, and deferred cases are distinguishable.

## Implementation Decisions

- **Approved ticket03 production exception:** the user explicitly approved a
  minimal standalone Management connection to IdP's existing owned Unix socket.
  Add an explicit absolute `identity_socket` selector, mutually exclusive with
  TCP `identity_database`, bounded connection/query deadlines and identity schema
  validation. Align stateless reset validation and docs. No automatic fallback,
  live-file reopening, migrations, new SQL owner, credential import, admission
  weakening, or replacement test transport is authorized by this exception.

- Inventory existing tests by assertions and dependencies. Classify them as private unit tests, owner tests, Unified-only integration tests, shared Node system scenarios, or deployment-specific system tests. Preserve meaningful assertions; remove duplication only when replacement coverage is identified.
- Use the existing public owner client/protocol seam as the system-test interface. The user approved this seam and generated-client-first consumption during the preceding discussion; no new interview is required.
- Place shared system scenarios outside the Unified implementation. Choose the smallest existing workspace test arrangement that can launch both layouts; do not introduce a production crate, framework, generic host trait hierarchy, or new public runtime API just to move tests.
- Keep public-interface Unified tests in its external integration-test directory. Keep private implementation checks beside the implementation. Move owner-only assertions to their owner without introducing direct server-to-server dependencies.
- Prefer small concrete fixtures and functions. Share behavioral assertions, not a replacement implementation of setup, admission, permissions, or replication.
- Model IdP, Management, Device, and Storage addresses independently. Do not assume one origin, fixed ports, or shared path prefixes. Preserve the distinction between address, canonical issuer, audience, Device identity, and endpoint identity.
- Launch the actual supported independent service processes, including required database-owner processes or documented local owner connections for the selected configuration. A child test process serving a hand-built substitute is not standalone full-Node qualification.
- Preserve legitimate setup differences. Unified coordinates one host operation; independent IdP, Device, and Storage workers require their own protected codes and documented correlated setup tuples. Management is not a fourth setup worker. Normalize verified deployment results for shared scenarios, not incompatible setup requests.
- The required independent layout is the documented supported same-machine configuration. Keep explicit remote-SQL attachment scenarios distinct from independently persisted Node Join/history import; shared database consumers are not evidence of offline-independent Nodes.
- Use generated HTTP clients and generated request/response models for all ordinary supported operations, including setup, login where covered by the contract, Device administration, permissions, and Storage operations. Configure actual service bases, trusted TLS, and appropriate credentials.
- If a supported operation is absent or incorrectly described, correct its OpenAPI contract and regenerate clients using the existing generation workflow. Do not add a handwritten supported-operation fallback or invent an endpoint to accommodate the tests.
- Use raw HTTP only for intentional wire-level cases such as malformed bodies, missing/duplicate headers, unsupported content types, unknown routes, or status/header details the generated client cannot expose. Keep such calls narrowly scoped and explain their purpose in test organization or naming.
- Use existing WebSocket/Iroh and resource-protocol clients where appropriate; generated HTTP clients do not replace those transports.
- Shared black-box assertions must not inspect private databases, set completion markers, inject ready state, or seed rows to bypass the behavior under test. Private fault-injection and storage-format assertions remain focused lower-level tests rather than being forced into the shared suite.
- Remove runtime fields, accessors, and fixture code that become obsolete. Do not widen production visibility solely to preserve white-box system tests.
- Preserve security, retained-state validation, admission, cancellation, publication fences, and recovery behavior. This is a test-architecture change, not a lifecycle redesign or journal migration.
- Keep protected-provider and OS-trust substitutions explicit and test-only. Do not equate mock-provider qualification with real OS installation/provider qualification.

## Testing Decisions

- A good system test verifies externally observable behavior through the same public clients/protocols a real consumer uses. It does not assert private method calls, SQL layouts, or internal handle ownership.
- Use one shared scenario body per common behavior, with separate executions for Unified and independent layouts. Report the layout in test names and failure context.
- At minimum, demonstrate fresh Create, verified readiness, ordinary login, an authorized owner/resource operation, retained-state restart, authorized Join, and completed Join reopen with the source stopped in both layouts. Check matching Installation/operation evidence rather than merely combining ready flags.
- Migrate applicable existing permission, synchronization, learned-denial, rejection, retry, and recovery assertions without silently dropping them. Document genuinely layout-specific or unsupported cases instead of disabling them or weakening expectations.
- Retain Unified-only shared-router/listener and owner-handle lifecycle coverage, and independent-only connectivity and process lifecycle coverage.
- Prior art includes existing external composition tests, hosted HTTPS setup and API Join scenarios, full-Node offline/restart scenarios, generated Rust and built JavaScript qualification, and remote-owner process fixtures. Reuse their valid scenario knowledge without copying their private coupling or parallel domain implementations.
- Generated Rust clients are sufficient as the default shared Rust test consumer. Preserve existing generated JavaScript qualification without multiplying every system scenario across SDK languages.
- Use isolated roots and ports, explicit protected-provider handling, bounded polling with last observed status, safe diagnostics, and teardown on success and failure. Account for process-global credential-provider state; serialize affected cases or isolate processes rather than relying on unsafe parallel execution.
- Start with focused migrated tests, then run basic affected-crate default/CLI checks and representative shared scenarios for each layout. Run formatting, dependency-boundary checks, and relevant generated-contract checks when touched.
- Do not run CRAP/coverage or cargo-hack feature-powerset testing for this effort. Do not claim full-suite, platform, or deployment certification from focused validation. Record exact commands, results, timeouts, and remaining gaps.
- Completion requires the classification/coverage map, external Unified integration tests, shared scenarios passing in both layouts, and removal of replaced tests and obsolete hooks. A file move alone or two separately maintained assertion suites is not completion.

## Out of Scope

- New product features, owner endpoints, compatibility routes, or lifecycle semantics.
- Moving domain behavior back into Unified, or hiding it in a new Unified-core module.
- Removing necessary validation, recovery checks, or negative tests to simplify the harness.
- Forcing every private recovery/fault-injection test to run against both deployment layouts.
- Requiring a fixed percentage reduction in files or collapsing tests into giant files.
- Exhaustive cross-layout source/target combinations, platform matrices, WAN/container/mTLS certification, real OS trust installation, GUI automation, or performance qualification.
- Replacing existing protocol clients, building a generic test framework, or requiring a full SDK-language matrix.
- CRAP analysis, coverage campaigns, feature-powerset testing, unrelated lint cleanup, or automatic commit/merge/push as part of publishing this spec.

## Further Notes

The previous production-ownership cleanup and merge are complete. This effort addresses remaining test placement, private coupling, and deployment parity; it is not a claim that earlier validation established full independent-service parity.

Ownership vocabulary and setup semantics follow the shared glossary, Node API contracts, and shared control-plane deployment documentation. A Node hosts all four contexts; a Unified Host is one deployment composition, not the definition of a Node.

The selected test seam was agreed in conversation: public generated HTTP clients and existing native protocol clients. Deployment launch/setup differences are real and must remain explicit. Any newly discovered inability to perform a normal supported operation should be recorded as a concrete contract or implementation gap rather than bypassed in fixtures.
