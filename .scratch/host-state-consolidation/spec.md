# Consolidate Unified Host state and endpoint secret ownership

Status: ready-for-agent

## Problem Statement

Unified Host setup currently spreads host configuration, admission, operation binding, credential references, and provisioning progress across multiple independently published files. Maintaining custom locking, pending-file handling, synchronization, and recovery rules for each record increases complexity and leaves related host updates in separate durable steps.

Setup admission combines a temporary setup capability with permanent workflow history. Even after retirement, its record retains the canonical operation, including administrator and Node choices. Those facts serve real retry and recovery checks today, but admission is the wrong owner for workflow persistence, and related facts also exist in provisioning journals.

Unified endpoint identity uses an application-managed raw private-key file even though independent Device setup already uses a secure provider. This duplicates secret persistence mechanisms and gives equivalent endpoint identities inconsistent storage and reset behavior.

Operators want a simpler local state model: a temporary setup code, durable host metadata in an existing SQL engine, and endpoint secrets owned by the secure store, without weakening restart safety or changing domain ownership.

## Solution

Give the Unified Host one private, local database using the existing `ofdb-sql` engine for host-owned bootstrap metadata. Separate setup credential handling from operation history: a minimal temporary private code file authorizes setup, while the database owns operation binding, attempt evidence, completion, and durable retirement.

Move generic secure-provider storage beneath IdP-specific signing-key repositories and use it for Unified endpoint identity. Store only exact provider references and public endpoint identity in host metadata. Remove the raw endpoint-key file implementation rather than retaining a fallback.

Keep IdP, Management, Device, and Storage data in their existing owner stores. Preserve the current external setup contracts, exact retry rules, independent-owner behavior, and fail-closed recovery. This is a storage and ownership rewrite, not a new onboarding flow or a distributed transaction system.

## User Stories

1. As a host operator, I want Unified Host metadata in one local database, so that I do not have to manage several overlapping setup journals.
2. As a host operator, I want only temporary credential material in the setup-code file, so that admission does not retain administrator and Node choices after setup.
3. As an Initial Administrator, I want setup requests authorized by the selected host's code, so that unrelated callers cannot provision the Node.
4. As an Initial Administrator, I want an accepted operation durably bound before provisioning starts, so that retries cannot change its identity or choices.
5. As an Initial Administrator, I want an exact retry to retain its operation nonce and allocated identities, so that interruption does not create a second Installation or Device.
6. As an Initial Administrator, I want conflicting Create and Join requests rejected, so that concurrent callers cannot change an in-progress setup operation.
7. As an Initial Administrator, I want authorized intent recovery after a browser reload, so that I can recover the selected operation without storing credentials in the browser.
8. As an Initial Administrator, I want cancellation to stop further eligible work without claiming rollback, so that retained owner changes are not mistaken for a fresh host.
9. As an Initial Administrator, I want setup completion and admission retirement recorded together, so that restart cannot expose a completed-but-authorized host due to separate host writes.
10. As a host operator, I want a leftover temporary code to grant no authority after retirement, so that failed code-file cleanup cannot reopen setup.
11. As a host operator, I want completed Create retries to retain their existing operation checks and normal administrator authentication, so that a retired code is not accepted as login authority.
12. As a joining Node operator, I want the exact trusted peer and authenticated issuer retained with the operation, so that restart cannot substitute another Installation.
13. As a Node operator, I want completed restart to use existing local state without contacting the original Join peer, so that my Node remains independently usable offline.
14. As a Node operator, I want endpoint private material stored in the secure provider, so that Unified does not maintain its own raw endpoint-key file.
15. As a Node operator, I want the same endpoint public identity after restart, so that existing Device bindings remain valid.
16. As a Node operator, I want missing, malformed, or inaccessible endpoint secrets to block startup, so that the system never silently replaces my Device transport identity.
17. As a host operator, I want provider failures distinguished from a genuinely absent entry internally, so that an unavailable provider does not trigger secret generation or file fallback.
18. As a host operator, I want attempted key creation recorded before provider work, so that a crash cannot turn an uncertain creation into a fresh attempt.
19. As a host operator, I want exact provider references retained for reset, so that cleanup does not depend on enumerating or guessing secret names.
20. As a host operator, I want interrupted reset to retain enough evidence for explicit continuation, so that failed secret deletion does not destroy the references needed to finish cleanup.
21. As a host operator, I want missing or corrupt host metadata refused without repair or regeneration, so that lost recovery evidence never reopens setup.
22. As a host operator, I want exclusive host ownership preserved, so that two processes cannot provision the same host concurrently.
23. As a Node operator, I want private host metadata excluded from domain replication and remote SQL access, so that setup capabilities and local recovery evidence remain local.
24. As an independent-owner operator, I want existing admission, credential handoff, and reset behavior preserved, so that shared-module changes do not break independent deployments.
25. As a maintainer, I want secure-provider storage reusable without depending on IdP domain repositories, so that transport identity does not acquire signing-identity ownership or bootstrap coupling.
26. As a maintainer, I want obsolete Unified file journals and raw-key handling deleted, so that there is only one authoritative implementation to maintain.
27. As a maintainer, I want focused lifecycle and secure-provider regression tests, so that this rewrite is verified without a full feature-powerset or complexity-analysis campaign.

## Implementation Decisions

- Use the existing `ofdb-sql` engine for a dedicated Unified Host database. The existing native engine uses redb/Automerge; do not introduce SQLite or another SQL dependency.
- Keep this database independent of IdP, Device, and Storage database instances. It must be available before domain provisioning and must not inherit an owner's replication, remote access, or reset lifecycle.
- Consolidate host-owned configuration, operation selection and canonical binding, allocated provisioning identities, credential/provider references, attempt and acknowledgement evidence, completion, and retirement. Keep owner-specific domain state and protected inventories under their current owners; do not copy their authority into host metadata.
- Use a small versioned schema with one authoritative representation of each host fact. Do not retain parallel JSON journals or add mirrors of legacy state. Choose record granularity based on independent updates and transaction needs, not the former file layout.
- Keep database creation restricted to a positively fresh host. Reopen must require existing valid schema and state; an existing host with a missing or corrupt database is not fresh. Establish fresh-host checks before any database operation that could implicitly create missing storage.
- Preserve process-level exclusive ownership. Reuse database exclusivity or existing lifecycle locking as appropriate; do not remove a required lock merely to reduce the file count.
- Separate setup capability authentication from workflow persistence. The temporary private code file contains only credential material needed to authorize setup, not operation choices, progress, or historical results. The host database owns durable binding and retirement.
- The setup code remains a reusable setup capability until retirement, not a normal sign-in credential or a promised single-use redemption token. Do not introduce expiry, rotation, or a new authentication flow in this rewrite.
- Preserve protected local code retrieval, selected-target authorization, existing header-based admission, and authorized nonsecret intent recovery. Raw codes and passwords must not enter URLs, public status, logs, debug output, domain history, or replication.
- Record the canonical operation and initial host attempt evidence atomically before privileged owner mutation or an accepted response. Conflicting operation tuples must fail; exact retries must use retained identities and choices.
- Commit host completion and admission retirement in the same host transaction after required owner publication and verification. Delete the temporary code afterward. Durable retirement takes precedence over any leftover code file, including after deletion failure or restart.
- A missing code while durable state still requires live admission must fail closed. A retired host must not require the removed code to reopen normally. Code creation interruptions must not silently mint replacement authority for an existing host.
- Preserve completed Create retry checks, normal Initial Administrator authentication, and exact completed Join checks by reading the retained host operation instead of admission-owned history.
- Preserve cancellation and finalization serialization, readiness verification, and the distinction between setup completion and current owner readiness. Cancellation is not rollback or proof of cleanup.
- SQL transactions cover related host facts only. Retain the external-effect sequence: commit attempted evidence, perform owner/provider/network work, then commit acknowledgement. Do not hold host write transactions across external waits or claim atomicity across separate stores.
- Share generic byte-secret storage and provider initialization below IdP-specific repositories, preferably in an appropriately feature-gated part of the key crate. Keep cryptographic derivation independent of platform storage requirements and preserve default `no_std` behavior where applicable.
- Keep endpoint identity lifecycle with Unified Host/Device. Do not route endpoint retrieval through IdP signing-key derivation, an authenticated remote owner, or an identity database that requires the endpoint to start.
- Reuse the existing secure-provider mechanism and independent Device precedent. Persist exact provider references and expected public endpoint identity; write attempted evidence before creation, verify provider material, and compare the public identity on reopen. Account for the absence of a create-only operation in the current raw provider interface rather than assuming writes cannot overwrite.
- Treat provider errors as errors, not missing entries. Never fall back to a raw file or generate a new endpoint identity when an attempted secret is missing, corrupt, or inaccessible.
- Update all Unified endpoint existence checks, public-ID checks, native/shared startup, reset, and reference-recovery behavior to use the new owners. Preserve exact secret deletion and interrupted-reset evidence before removing local metadata. Do not delete shared or remote owner data.
- Remove superseded Unified file publication, parsing, pending-record handling, and raw endpoint-key code once their responsibilities move. Retain genuine configuration, TLS, and other file artifacts that remain outside this consolidation.
- Do not implement automatic legacy migration, dual reads/writes, historical serving wrappers, or raw-key fallback. Detect unsupported existing layouts before creating replacement state and leave them intact with an actionable refusal. Retain required explicit reset behavior without adding general-purpose compatibility infrastructure.
- Update existing setup, security, operations, and native lifecycle documentation to describe authoritative ownership, recovery, and the intentional legacy-layout break. Assess complexity and risk qualitatively before implementation; the highest risks are accidental fresh creation, identity replacement, and loss of reset references.

## Testing Decisions

- The agreed primary test seam is the existing Unified Host lifecycle interface, using its Create/Join integration tests. Test observable authorization, returned operation/status, endpoint identity, restart behavior, and cleanup outcomes rather than filenames, SQL table names, or serialized journal formats.
- Extend existing lifecycle scenarios for fresh Create and Join, exact and conflicting retries, concurrent submissions, authorized intent recovery, cancellation, retirement, completed retries, ordinary restart, and current readiness failures.
- Verify that operation binding is durable before accepted work or owner mutation, and that related host updates either commit together or remain at the previous valid state.
- Verify completion and retirement atomicity, including failure before commit and a committed retirement followed by failed temporary-code deletion. A leftover code must never authorize setup.
- Exercise fresh-code creation interruption, missing live code, missing/corrupt host database, unsupported schema, legacy layout detection, and competing host ownership. None may cause regeneration, silent repair, or new setup authority.
- Use existing injected/mock secure-provider seams for creation, readback, missing/corrupt material, public-ID mismatch, provider unavailability, overwrite uncertainty, and deletion failure. Verify that endpoint identity is stable across reopen and no raw-key fallback is created.
- Exercise interruption between attempted evidence, external effects, and acknowledgements. Verify exact continuation or explicit refusal without replacement identities or claims that external writes rolled back.
- Verify completed reopen without original Join-peer access and preserve the distinction between endpoint identity, approved Device membership, User authentication, and resource permission.
- Extend existing reset and native/shared lifecycle tests for exact provider references, failure before provider deletion completes, explicit continuation, and refusal to remove unrelated or shared owner state.
- Run focused independent-owner admission/provider regressions where shared modules change. Do not rewrite independent provisioning persistence merely to share Unified storage.
- Prefer existing host integration tests, admission authorization/restart tests, endpoint publication fault tests, independent Device provider tests, and native reopen/reset tests as prior art. Add lower-level tests only where an essential fault or transaction outcome cannot be exercised reliably through those seams; no new public test-only interface is required by this spec.
- Use isolated temporary roots and injected providers; do not modify real host data, OS secret stores, or OS trust during automated validation. Mock-provider results do not certify production persistence or reboot behavior of every platform backend.
- Validation is targeted: run the affected lifecycle and provider suites, necessary compile checks for affected feature gates, and formatting/lint checks scoped to changed code where practical. Explicit user direction excludes a full `cargo hack test --feature-powerset --all-targets` run and `just crap` from acceptance. Record exact commands, failures, and limits; do not claim unrun validation.

## Out of Scope

- Replacing the existing database engine with SQLite or introducing a second SQL engine.
- Merging IdP, Management, Device, and Storage ownership or database lifecycles.
- Moving all Storage catalogs, resource persistence, independent-owner journals, configuration, TLS artifacts, or filesystem data into SQL.
- Storing endpoint private keys as SQL metadata or retaining an application-managed raw-key fallback.
- Changing endpoint cryptography, Device approval, token-signing identity, resource authorization, or Join trust discovery.
- Global transactions or two-phase commit across owner databases, providers, networks, or trust installation.
- Setup-code expiry, rotation, normal sign-in redesign, new discovery protocols, or new onboarding UI.
- Automatic migration or continued serving from legacy Unified file layouts.
- New production qualification across real provider installations, reboot persistence, containers, WAN conditions, or every supported platform.
- Full feature-powerset validation and CRAP/complexity-tool validation.

## Further Notes

The design was synthesized from source and documentation investigation; no implementation has been changed and no runtime validation was performed during that investigation. The user confirmed the lifecycle/provider testing seams and explicitly excluded full cargo-hack and CRAP testing.

The current admission payload has real consumers: operation conflict prevention, authorized recovery, completed Create retry correlation, and completed Join validation. Removing that payload is safe only after those consumers use host-owned metadata. Likewise, SQL removes redundant publication machinery but does not remove the need for durable attempted/acknowledged phases around external effects.

The secure store may itself persist secrets according to the selected platform backend. The guarantee here is removal of application-managed raw endpoint-key files, not hardware-backed non-exportability or proof that private material never reaches physical storage.

Existing deployments require an intentional operator decision before adopting this breaking storage rewrite. Implementation must leave unsupported retained state intact; do not reset, delete, or migrate an operator's data automatically.
