# Simplify Installation Setup and desktop startup

Status: open

## Current acceptance scope (2026-10-28)

The current milestone is **a working single-device experience: one device undergoing setup at a time**, primarily the default bundled desktop. Qualify Create through ordinary sign-in, returning-user startup, interruption/exact retry, cancellation, and retained-identity reopen. Join remains in scope as one joining device with its approving source device; “single-device” does not remove authenticated source approval or require Join to work without a source during initial setup.

Simultaneous independent Create/Join operations and default-concurrency test-suite throughput/reliability are **deferred**, not acceptance blockers for this milestone. Running independent scenarios sequentially is valid evidence for this explicitly narrowed scope, not proof of parallel reliability. Preserve the existing concurrent failures and diagnostics as unresolved follow-up evidence; do not report them as fixed or the default-concurrency suite as passing. No request/readiness deadline, security assertion, or production behavior is changed by this scope decision.

Ticket [07](issues/07-qualify-simplified-workflow.md) is **resolved for the automated single-device workflow scope**. [Single-device qualification evidence](evidence/07-single-device-qualification.md) records the complete sequential owner suite **44 passed / 0 failed / 6 existing child-entry ignores**, fresh unified HTTPS Join **3/3**, and setup HTTP client **24/24**. Existing mounted/injected evidence is reused at its stated seams; no live GUI or release pass is inferred.

Ticket [09](issues/09-diagnose-readiness-and-request-timeouts.md) remains **open, deferred** for parallel independent workloads; resume active diagnosis if a failure reproduces in the single-device path. Ticket [11](issues/11-complete-provisioned-validation.md) remains **open / ready-for-human**: [bounded validation evidence](evidence/11-single-device-validation.md) records **4/10 root feature combinations completed successfully**, the fifth interrupted at the **7200s** ceiling, and five unrun. Separate workspace CRAP stopped during instrumented compilation, **before tests**, when available RAM fell below **2 GiB**; no fresh coverage report or CRAP score exists. These sequential results do not establish parallel reliability or full-matrix/workspace completion.

**This spec remains open** for ticket 11's broad validation and actual desktop qualification limits: packaged desktop/GUI, production provider, real OS trust/elevation, physical/WAN and platform/release checks remain unqualified. Next actions are human coordination of sufficient sustained resources and an authorized time budget for the broad gates, plus actual desktop qualification before release—not reopening ticket 07 absent a reproduced in-scope failure.

## Previous integration status (2026-10-27; historical)

**Tickets 07, 09, 11 and this spec remain open.** Implementation tickets 01–06 and follow-ups 08/10 are integrated. Reviewed fixes include shared-worker/queued-reply and independent connection destruction fences; Storage identity-first rejection with current-policy/response rechecks retained; Management policy-schema, standalone setup HTTP, and continuous source-HTTPS-listener fixture repairs. The Device conflict fixture now imports its initial complete catalog/data through the public sync session; its focused suite passes **6/6**, with security-negative assertions unchanged. Optional setup-secret lock diagnostics are observation-only. Application-owned AGENTS guidance from `1cfe76076` is included, not a qualification result.

Sibling `ofdb/integration/setup-simplification-followup` is **`a8c23a54540e591aa024396a5674dc016b6cce3a`**: outgoing candidate-observation batching, exact index-field point reads, measured schema execution-stage stack boundaries, and operation-local same-transaction schema reuse. Each has recorded focused causal/semantic controls; these passes are not pooled into whole-workflow acceptance. See [09 execution evidence](evidence/09-timeout-localization.md) and [Storage/stack controls](evidence/09-storage-denial.md).

The [latest supported root matrix](evidence/11-validation-attempt.md#supported-qualification-on-current-inputs-2026-10-27) attempted **1/10 combinations: 36 passed / 8 failed / 6 ignored**, zero successful combinations, nine unrun. Workspace instrumented compilation completed, then `device_contract` failed **5/1** on incomplete catalog import. The [fixture repair's 6/6 focused pass](evidence/11-device-contract-fixture.md) does not replace that result: **coverage/CRAP and the full matrix have not been rerun**, and no fresh coverage report or score exists. Neither supported gate hit a resource/time stop. A separate lock-diagnostic bootstrap parent passed **78.2s**, with **282 acquisition/release pairs and zero contention**; it does not explain the historical initial-driver WouldBlock or prove full-suite reliability. Remaining gates are current readiness/nested authorization timeouts, actual contention-holder localization, then unchanged-bound integrated matrix/coverage qualification and real GUI/platform/provider/trust/WAN acceptance. Earlier dated evidence remains historical; no acceptance box is newly completed.

## Problem Statement

Getting started requires users and callers to understand too many technical distinctions: native startup, service selection, setup admission, Installation creation or Join, owner readiness, Device Setup completion, and recovery. The normal path feels clunky and exposes implementation concerns instead of guiding the user toward a usable Application.

The desktop exposes separate commands for host addresses, setup stage, public login Client, startup errors, setup capability retrieval, completion, retry, reset, reference recovery, and peer revocation. Its setup stage combines shared readiness with a separate Device completion marker. Startup waits for host preparation, OS certificate trust work, and HTTPS verification before building the window. Recovery mechanisms share the setup interface even though they are exceptional maintenance operations.

The problem is workflow complexity and duplicated coordination, not merely unattractive screens or too many Rust files. Simplification must not remove authorization, destroy retained identity, silently replace an interrupted operation, or mistake request acceptance for readiness.

## Solution

Make the default bundled desktop experience a short guided flow with one initial choice: **Create a new Installation** or **Join an existing Installation**. Use sensible defaults, hide internal identifiers and service composition, show progress, and proceed to ordinary sign-in or the Application when current readiness is verified. Do not add automatic authentication or bypass ordinary sign-in.

Create asks only for required Initial Administrator credentials. Node naming defaults automatically and can be edited later. Join uses one trusted peer input; an invitation/link is desirable only if it can preserve the existing authenticated peer-pinning contract without introducing an unrelated pairing protocol. Source-side acceptance and authorized approval remain explicit, but are presented as one understandable add-device task.

Show the desktop window before slow host work. Present a coherent onboarding snapshot and actionable progress/errors rather than making the UI assemble multiple independent status reads. Keep local data selection editable after Installation Setup; it must not artificially block ordinary sign-in when safe defaults are valid.

Keep supported remote-service and browser workflows available through a clearly separated advanced connection/configuration path. Connecting to existing services is not Join and must not provision the bundled Node. Place destructive reset and exact reference recovery in maintenance rather than the normal onboarding path.

## User Stories

1. As a new user, I want a clear choice between creating an Installation and joining one, so that I know how to start without understanding host internals.
2. As an Initial Administrator, I want to supply only the credentials required to create an Installation, so that I do not configure internal identifiers or service relationships.
3. As a new user, I want a sensible default Node name, so that naming does not interrupt setup.
4. As a User, I want to rename my Node later, so that accepting the default is not a permanent compromise.
5. As a joining user, I want one trusted peer input, so that I do not separately enter issuer, endpoint, and technical addressing data already derivable through authenticated discovery.
6. As a joining user, I want the target Installation identified before membership is finalized, so that I can avoid joining the wrong Installation.
7. As a joining user, I want clear instructions when another device must approve my request, so that I know what action is required and where.
8. As an authorized existing User, I want acceptance and exact pending-request approval presented together, so that adding a Node does not require navigating unrelated technical controls.
9. As an authorized existing User, I want to inspect and reject an unexpected request, so that simpler onboarding does not weaken membership approval.
10. As a new user, I want one progress screen during provisioning, so that internal owner phases do not become additional choices.
11. As a user, I want accepted work to continue if I close a page, so that navigation does not discard a durable operation.
12. As a returning user, I want interrupted setup to recover the same operation, so that retries do not create another identity.
13. As a returning user, I want any required credentials requested again rather than persisted in the browser, so that recovery does not leak setup secrets.
14. As a user, I want sign-in offered only when the selected Node is currently ready, so that successful submission is not confused with a usable Installation.
15. As a User, I want local data-selection preferences editable after setup, so that optional configuration does not delay basic use.
16. As a User, I want default local storage behavior to preserve existing restrictions and permissions, so that skipping a preferences screen does not broaden access.
17. As a returning user, I want the Application to open without replaying onboarding, so that startup is routine.
18. As a desktop user, I want a visible window while the host starts, so that startup does not look frozen or absent.
19. As a desktop user, I want certificate trust consent explained before elevation, so that I understand the OS change being requested.
20. As a desktop user, I want already satisfied certificate trust checked without reinstalling it, so that normal launches avoid unnecessary privileged work.
21. As a user, I want transient failures to offer a clear retry, so that I do not need to understand runtime lifecycle commands.
22. As a user, I want protected-provider failures to explain what prerequisite needs attention, so that I am not encouraged to reset recoverable data.
23. As a user, I want damaged retained identity reported as requiring repair, so that the Application never silently creates a replacement Installation.
24. As a remote-service user, I want my selected service addresses honored, so that the desktop never silently substitutes bundled services.
25. As a remote-service user, I want connecting or signing in distinguished from joining as a Node, so that I do not accidentally provision local owners.
26. As a browser user, I want the shared setup flow to work without native commands, so that simplification does not make Tauri a prerequisite.
27. As an operator, I want reset accessible through maintenance with explicit confirmation, so that it does not clutter onboarding or become a default repair action.
28. As an operator, I want failed credential deletion to preserve the remaining local evidence and data, so that reset can be retried safely.
29. As an operator, I want local deletion and remote revocation reported separately, so that I know whether a surviving peer still needs to acknowledge removal.
30. As a maintainer, I want one typed onboarding observation contract, so that screens do not reconstruct lifecycle state from fragmented reads.
31. As a maintainer, I want the shared worker to remain the sole Create/Join implementation, so that desktop and browser behavior cannot drift.
32. As a maintainer, I want obsolete commands, markers, screens, and duplicate tests deleted when replaced, so that simplification actually reduces code.
33. As a security maintainer, I want admission, ordinary User authorization, peer trust, and resource permission kept distinct, so that a shorter workflow does not become an authorization bypass.

## Implementation Decisions

- Complexity/risk/priority: high priority; medium workflow/status complexity and high security/lifecycle risk. Prefer deletion and consolidation over new orchestration layers or generic recovery frameworks.
- Preserve domain ownership. A Node hosts IdP, Management, Device, and Storage. Installation Setup creates or joins an Installation. Device Setup configures locally stored data; it is not membership approval, authentication, or proof of current readiness.
- Keep the shared HTTPS setup worker authoritative for durable intent, provisioning, admission retirement, cancellation, and readiness. Tauri owns platform prerequisites and bundled host lifecycle, not a second Create/Join implementation.
- Establish one typed presentation snapshot for onboarding. Its conceptual states are starting, needs setup, working, ready, and blocked. Working may carry an approval instruction; blocked carries a safe reason and supported action. This is a projection of authoritative evidence, not a new persisted state machine or journal.
- Consolidate fragmented frontend observation behind this interface. Preserve selected-target identity and distinguish bundled-host failure from the state of remotely selected services. A snapshot must not contain setup capability secrets. Do not blindly proxy every remote HTTP operation through Tauri.
- Keep privileged setup capability retrieval separate and scoped to protected local management. Move admission-state parsing into the owning shared module rather than duplicating its file format in the desktop.
- Use typed shared responses rather than converting them to untyped JSON to inspect readiness fields. Do not infer readiness from an available listener, historical marker, accepted request, successful admission, or sign-in.
- Default bundled onboarding to Create or Join. Move existing service composition and remote connection choices to an advanced path without removing their supported behavior. Browser and independent-owner setup must retain exact owner-specific admission and correlated readiness checks.
- Default the Node name without adding a dependency; retain an ordinary later editing path. Remove other optional onboarding inputs only after confirming they can be derived or deferred safely.
- Keep the existing single pinned-peer Join input as the minimum supported contract. Prefer an invitation representation only when it can carry validated existing peer evidence. A new invitation issuance, expiry, or redemption protocol requires separate design and is not an assumed prerequisite for this spec.
- Authenticated discovery already derives the issuer. Do not add manual issuer entry or trust arbitrary discovered peers, unsigned metadata, or an invitation's unverified claims.
- Source approval uses ordinary authorized owner routes, with exact pending-offer inspection and rejection. Hiding owner phases does not eliminate required approval or conflate Device approval with independent signer enrollment or resource access.
- Retain operation identity across observation, transport timeout, restart, and exact retry. Recovery may require re-entry of transient credentials. Do not promise automatic resume where the existing authorization contract requires an explicit authorized retry. Closing a page is not cancellation; cancellation acknowledgement is not completed cleanup.
- Remove the mandatory Device-completion click/marker if it only records wizard dismissal. Audit callers and local data-selection behavior first. Defer genuine optional Device Setup to settings; preserve any required owner operation and validate safe default behavior. Do not delete the Device Setup domain concept or treat default preferences as additional authorization.
- Open the desktop window before slow startup work. Run startup asynchronously with serialized start/stop/retry/reset transitions, observable progress, and safe handling of shutdown during startup. Keep native entry modules thin.
- Verify retained certificate trust before deciding installation is needed. Request explained, explicit consent for necessary installation; build production verification clients at the appropriate point after trust changes. Preserve retained certificate identity and native-root HTTPS verification. A success marker alone cannot waive current trust verification.
- Keep desktop HTTPS and the current security model in this effort. An app-private transport redesign is a separate decision, not an implementation shortcut.
- Separate runtime retry, destructive maintenance, and advanced exact-reference recovery in presentation. Keep reset resumable, stop/join the host before deletion, preserve evidence through provider failure, and retain public revocation work after local deletion.
- Move shared provisioning-reference knowledge toward its owning module where doing so deletes duplication. Do not rewrite proven reset internals solely to reduce file count.
- Delete superseded commands, completion persistence, routes, stores, screens, and redundant tests as replacements land. No compatibility wrappers, silent migrations, replacement identity generation, or unrelated dependency additions.
- Update stable domain/design, interface, operations, and native handoff documentation where behavior changes. Do not preserve contradictory historical workflow instructions as current guidance.

## Testing Decisions

- Qualify one device undergoing setup at a time. Execute independent lifecycle scenarios sequentially and record scheduling, exact inputs, commands, and outcomes. Internal owner coordination and lifecycle contention within that device remain in scope; parallel independent setup stress does not. A failure reproduced in this single-device scope remains a blocker.
- Test externally observable workflow and authorization behavior, not command counts, private helper structure, JSON projection internals, or incidental file layout.
- Prefer the existing rendered setup-page seam for onboarding acceptance: operate visible controls, supply existing HTTP/native adapters, and assert the resulting screens and permitted actions. This is the highest practical seam for the user's complaint. Do not introduce a second orchestration-only fake framework.
- Reuse the existing shared HTTPS Create/Join lifecycle seam for durable identity, authenticated approval, exact retry, current readiness, and source-offline reopen. Keep native trust/provider injection as a narrow platform seam because a rendered-page test cannot qualify actual process ownership or OS trust.
- These seams are proposed from existing test prior art; no additional user confirmation was collected during this synthesis. Their use does not imply prior GUI or production-platform acceptance.
- Prior art includes rendered Installation setup and auth-layout tests, shared setup-client HTTP contract tests, native one-listener/trust-failure tests, isolated provider lifecycle tests, exact reset/recovery tests, and shared production HTTPS Join/offline-reopen scenarios.
- Cover fresh Create with only required inputs, default naming, progress, verified readiness, and ordinary sign-in. Cover Join input validation, authenticated issuer discovery, source acceptance/approval/rejection, completion, and offline reopen.
- Cover interruption after durable acceptance, reload/restart, exact intent recovery, required credential re-entry, retry without replacement identity, and cancellation without premature success.
- Cover deferred Device Setup: sign-in is not blocked by wizard dismissal; editing local data preferences remains possible; existing restrictions, selections, and authorization are not broadened.
- Cover bundled versus remote selection, including local startup failure with an otherwise usable remote target. Remote setup must never receive the bundled setup code or provision bundled owners. Browser setup must not invoke native commands; independent owners must retain separate admission scopes.
- Cover visible startup before host completion, already-trusted launch without unnecessary installation, consent refusal, trust-verification failure, retry, and close/reset contention during startup. Use injected commands and isolated roots/providers, not real profile or OS trust changes.
- Preserve negative coverage for unauthorized Create/Join, incorrect Origin, wrong issuer/peer, secret disclosure, malformed or missing readiness evidence, and lost retained host/TLS/provider state. An old completion marker must never manufacture readiness.
- Preserve reset safety cases: provider refusal retains data, exact reference recovery never overwrites conflicts, data-only cleanup does not repeat successful secret deletion, and remote revocation is acknowledged only after the required owner operations.
- Start with focused changed-area tests, then formatting, relevant frontend checks and Rust clippy. Final implementation validation includes `cargo hack test --feature-powerset --all-targets` and `just crap`; record commands, outcomes, failures, and qualification limits truthfully.
- Check the existing deployment-independent system-test effort before moving or duplicating lifecycle scenarios. Reuse its public-interface scenarios where available rather than creating competing suites.

## Out of Scope

- For this milestone: simultaneous independent device setup, default-concurrency suite reliability, and parallel-workload performance optimization. Their known failures remain unresolved follow-up work, not waived security or durability requirements.

- Replacing domain-owner authorization or introducing a global authentication-enable switch.
- Removing browser support, remote connections, or independent-owner deployments to simplify the bundled desktop screen.
- A new pairing/discovery/invitation protocol, general QR infrastructure, or zero-trust-input automatic peer selection.
- An automatic authenticated session after Create, a bootstrap-only sign-in bypass, or persisted Initial Administrator passwords.
- Plaintext desktop fallback, disabled certificate verification, custom production root bypasses, or a new app-private transport architecture.
- Automatic repair, migration, replacement keys/identity, remote erasure, or treating local reset as remote revocation.
- A general reset framework, mandatory rewrite of reset journals, automatic OS CA removal, or platform expansion beyond currently supported native behavior.
- A whole-Application visual redesign, unrelated code cleanup, new dependency frameworks, or production/platform certification inferred from injected tests.

## Further Notes

### Delivery order

1. Trace frontend callers and shared worker contracts. Record the required inputs, Device Setup semantics, safe defaults, target-selection behavior, and a small action/failure table. Resolve code-verifiable uncertainties from the repository rather than interviewing the user again.
2. Consolidate typed observation and lifecycle coordination; replace callers and delete superseded observation paths.
3. Remove the artificial Device-completion gate where verified redundant; retain actual local data-selection functionality outside mandatory onboarding.
4. Simplify Create, Join, and source approval presentation against the existing shared worker. Preserve advanced remote and browser paths.
5. Make startup visible/asynchronous and avoid unnecessary trust installation without weakening verification.
6. Isolate maintenance presentation, remove obsolete code, update current documentation, and run final validation.

### Acceptance criteria

Apply these criteria to one device undergoing setup at a time, including the source device needed to approve a single Join. Do not require parallel independent scenario qualification to accept this milestone.

- Default bundled first launch offers one essential Create/Join choice and no mandatory internal service configuration.
- Create uses required administrator credentials and a default Node name; Join uses one trusted peer input with clear source approval instructions.
- No mandatory completion-only screen remains between current verified readiness and ordinary sign-in.
- Startup is visible before slow platform prerequisites; existing verified trust avoids unnecessary installer invocation.
- The frontend renders one coherent typed onboarding observation instead of independently reconstructing lifecycle state.
- Required transient credentials stay out of URLs, logs, status, and frontend persistence.
- Interrupted work retains the same intent and identities; returning ready Nodes reopen without their original source peer.
- Remote selection is honored, is not mislabeled as Node Join, and cannot accidentally provision the bundled Node.
- Retry and maintenance are distinct; reset failures preserve evidence and remote revocation remains explicitly pending until acknowledged.
- Deleted workflow paths are actually removed, validation outcomes are recorded, and unchanged security guarantees remain covered.

### Evidence and uncertainty

This spec synthesizes the conversation and a targeted review of the supplied native code, shared glossary, current setup/security/operations contracts, native handoff documentation, and existing test interfaces. It does not claim a complete rendered-UI review or live GUI validation. Existing documentation already establishes authenticated issuer discovery and remote-service selection; these are not new features to rebuild. Some operational documentation differs from the supplied native implementation in detail, so implementation must verify current code rather than copy historical receipts as requirements. No application code or system trust state was changed to publish this spec.
