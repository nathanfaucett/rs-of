# Unified Host responsibility audit and qualification

## Result

Implementation complete on `refactor/unified-server-boundaries`. Independent static review accepted the production changes after three findings were fixed. Broad automated qualification is incomplete, as recorded below; this is not a workspace-wide pass.

## Retained Unified responsibilities

| Module | Retained responsibility and owner seam |
| --- | --- |
| Configuration | Loads and combines owner settings, normalizes host paths, enforces nonoverlapping host roots and listener constraints. Identity setting validity delegates to IdP configuration operations. |
| Bootstrap configuration | Retains nonsecret host selection, port, root, issuer/configuration references and process lock. Reset coordinates exact owner reference capture and owner secret deletion; it does not delete arbitrary owner keys or interpret database layout. |
| Credential bootstrap | Retains exact configuration references and the TLS-production attempt fence, installs/opens host TLS through the existing API module. Credential validation and private material remain owner behavior. |
| Local Create | Orders concrete owner operations and preserves owner handles through publication. Its journal is a cross-owner attempted/acknowledged construction fence, not a domain authority. Password verification and signer evidence belong to IdP; retained administrator inventory belongs to Management; acknowledged stores are checked by their owners. |
| Local Join | Coordinates endpoint construction, owner inventory, Device's public-bootstrap operation, owner-handle transfer and host publication. Exact IdP receipt/signer/import progress remains IdP-owned; Device implements protocol and readiness. |
| Setup host | Composes protected Setup admission, TLS and worker serving; owns listener and task lifetime. |
| Setup runtime | Adapts the existing HostedSetup worker to concrete owner construction/reopen/publication. Retained host phase checks prevent accidental fresh initialization; owner evidence still decides readiness and domain validity. |
| Endpoint | Persists the host's one endpoint identity and constructs it with the owner-provided admission hook. Approved membership and issuer discovery come from owner operations. Key publication/failure tests remain host-lifecycle tests. |
| Security facade | Constructs Device's protocol owner, attaches its stream handler to Storage's dispatcher, and transfers existing handles into runtime construction. No replication handshake, historical validation implementation, repository lookup, or admission state machine remains. |
| Runtime | Opens owners through their constructors, injects local owner interfaces, mounts routers, registers handlers, subscribes to changes and coordinates startup/shutdown. Convenience methods delegate to owners; owning an operation's caller is not owning its policy. |
| Bootstrap verification | Probes composed public owner contracts and readiness through the real listener and shuts the runtime down. It does not define domain readiness. |
| CLI and crate roots | Argument parsing/dispatch and declarations/re-exports. No new feature, crate, generic orchestration framework, or file-count target. |

## Extracted behavior

- Device owns scoped security synchronization, purpose-bound public bootstrap, Device admission, membership and readiness composition. Current authorization remains before outbound sends and incoming applies.
- IdP's `LocalIdentity` owns exact enrollment/current signer/active-User evidence; Device no longer constructs IdP repositories to make those decisions.
- IdP's provisioner supplies retained approval/import-journal operations through the existing purpose-specific owner seam. Journal names, exact anchors and protected-key verification remain intact.
- IdP owns public operator-login registration construction, retained password verification, signer binding and prepared/approved evidence, identity settings validation, local identity-store construction/migration, and existing-only preflight.
- Management owns retained initial-administrator inventory and its permission/action mapping in the identity store.
- Device owns Device-store construction/migration and existing-only preflight; Storage owns retained Storage-root checks. Unified no longer embeds owner database filenames or schema migration calls in production paths.
- Existing `db` utilities provide bounded SQL stream framing without a Device-to-Storage-server dependency.

## Test responsibility decisions

- Keep real cross-owner Create/Join, setup/recovery, public readiness, permissions, copy retirement, offline restart, and learned-revocation scenarios. Their assertions test composed behavior, not just a module name.
- Move outbound security protocol tests with Device's implementation; add owner contract tests for protocol admission, IdP identity/settings, retained credential/signer evidence, Management grant inventory, and existing-only preflight.
- Delete the old bootstrap test's parallel SQL exchange/validation implementation. Existing persistence-owner tests cover generic directional native reopen/retry. The real Device bootstrap receiver now supplies bootstrap-specific exact retry, unchanged source history, and no reverse-publication coverage.
- Split the full-node test handle helpers into explicit identity and Device accessors. Management fixtures use identity policy, Device fixtures use the Device store.
- Correct stale fixtures to current Device routes, canonical owner intents and Management-audience approval tokens. Pending Join publication is observed through a bounded authorized owner API poll after the raw offer, not assumed to precede it. Expected security outcomes are unchanged.
- Generated SDK qualification retains canonical export assertions. Clean/rebuild all four owner SDKs before testing built exports; stale output is not an alternative supported interface.

## Final validation observed

All integration Cargo jobs were serialized. Focused Unified test builds used the command-line-only `profile.test.package.unified-server.opt-level=0` override and `RUST_MIN_STACK=16777216`; no production profile was changed.

Passed on the integration branch:

- `cargo check` for Unified, IdP server/service, Device server/service, Storage server, Management server and `idp-db`, with all targets/features, offline. Rerun after all production changes and final fixture corrections.
- Strict affected-library Clippy with all features, `--no-deps`, offline and `-D warnings`.
- Device no-default-feature check. IdP dependency emits two unused-import warnings in untouched OAuth modules under this configuration; no new std-only Device module is enabled by default.
- Workspace formatting, server dependency-direction check, and diff whitespace checks.
- Partial credential Create/Join recovery: 1 passed after canonical intent fixture correction.
- Three complete two-host scenarios: offline partition/reopen/learned denial, retained-socket restriction propagation, and grant consumption/revocation reconciliation. 3 passed in 381.11 seconds using the final no-default-feature test binary built by the feature-matrix attempt.
- Real policy HTTP/copy-retirement/reselection/transfer scenario: 1 passed.
- Setup route publication/unavailability lifecycle: 1 passed.
- Generated Rust and built JavaScript owner operations: 1 passed after clean ESM builds of IdP, Device, Management and Storage SDKs. No generated source or build artifacts are committed for this qualification.
- The broad serial run also observed successful host endpoint publication/interruption tests, generated transport denial tests, production HTTPS API Create/offline reopen, HTTP setup/reopen, and the single-host refresh/replay/restriction probe before its timeout.
- Ticket answers record additional focused owner, actual protocol, interruption/recovery and offline Create passes. They are scoped results, not a completed feature matrix.

Incomplete or not green:

- The first all-feature Unified library run timed out at 600 seconds and exposed fixture failures. The reproduced partial-credential and three full-node failures were repaired and the affected tests passed afterward. The complete all-feature library suite was not rerun to completion.
- `cargo hack test -p unified-server --feature-powerset --all-targets --offline --config profile.test.package.unified-server.opt-level=0 -- --test-threads=1` timed out at 600 seconds in the first configuration after successful tests, including the repaired two-host offline scenario. The powerset is not qualified.
- `RUST_MIN_STACK=16777216 just crap` timed out at 600 seconds compiling instrumented workspace/application tests. No complexity/coverage report was produced.
- Earlier ticket all-target Clippy reported unchecked read amounts in untouched Storage test fixtures. Strict library Clippy is green; all-target Clippy is not claimed green.

## Review

The requested change-review skill was unavailable. An independent read-only code review identified three concrete gaps: IdP evidence interpreted by Device, identity settings validated in Unified, and a parallel SQL bootstrap test implementation. All three were fixed in `d8a0e21c1`; follow-up static review accepted those fixes and the retained host responsibilities. Documentation API-name/scope inaccuracies identified by that review were corrected before finalization.

Remaining qualification is bounded testing/coverage work, not an identified remaining ownership violation. Do not describe this implementation as fully qualified across all features or deployments.
