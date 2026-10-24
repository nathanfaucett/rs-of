# 07: Qualify the simplified workflow and remove remaining obsolete paths

**What to build:** The simplified experience works as one complete flow across Create, Join with source approval, returning-user startup, interruption, advanced remote connections, browser setup, and maintenance. Reviewers receive reproducible evidence and an honestly scoped validation report.

**Blocked by:** 02 — Make Create lead directly to ordinary sign-in; 03 — Make Join a single guided request; 04 — Make adding a Node understandable on the approving device; 05 — Show startup immediately and request trust only when needed; 06 — Separate maintenance from normal onboarding.

**Status:** ready-for-agent

- [ ] Verify default bundled first launch has one essential Create/Join choice, minimal required input, no mandatory internal composition, and no completion-only gate before ordinary sign-in.
- [ ] Exercise the integrated Join flow from request through source acceptance and exact approval/rejection to verified readiness; verify completed reopen without the source peer.
- [ ] Verify accepted work survives disconnect and restart, exact retry retains identities, required transient credentials are requested again, and returning ready Nodes do not replay onboarding.
- [ ] Verify visible asynchronous startup, already-trusted launch, consent refusal, actionable prerequisite failure, and lifecycle contention together with the final setup routing.
- [ ] Verify selected remote services remain usable without local provisioning or bundled credential forwarding, browser setup never requires Tauri, and independent owners retain separate admission and correlated readiness checks.
- [ ] Verify optional local data configuration remains editable without changing restrictions or resource authority, and maintenance reports local deletion separately from incomplete remote revocation.
- [ ] Retain negative coverage for authorization, Origin, wrong peer/issuer, secret disclosure, malformed readiness evidence, and lost identity/TLS/provider material. No historical completion marker can manufacture readiness.
- [ ] Reuse existing rendered-page and public HTTPS lifecycle scenarios and coordinate with the deployment-independent system-test effort; do not duplicate its scenarios or infer GUI coverage from backend tests.
- [ ] Inspect the final call graph and remove any remaining superseded commands, markers, routes, stores, screens, wrappers, or duplicate tests belonging to this workflow. Each preceding ticket must already include its own focused tests and local cleanup.
- [ ] Reconcile current glossary-linked design, interface, operations, and native handoff guidance with delivered behavior; clearly distinguish historical evidence from current instructions.
- [ ] Run relevant frontend checks/tests, Rust formatting and clippy, then final `cargo hack test --feature-powerset --all-targets` and `just crap`. Record actual commands, outcomes, existing unrelated failures, and any unavailable tooling without claiming unrun checks passed.
- [ ] Record which rendered/integration scenarios actually ran and their limits. Injected trust/providers do not certify real OS trust installation, physical/WAN pairing, unsupported platforms, or production deployment. Do not modify real profiles or OS trust stores to satisfy automated validation.
