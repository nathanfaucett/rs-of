# 03: Enforce shared-contract generation and document the workflow

**What to build:** Contributors have one documented, runnable check that verifies complete API contracts and reproducible SDKs, and CI rejects contract or generated-client drift without requiring a running deployment.

**Blocked by:** 02: Replace handwritten HTTP calls with generated clients.

**Status:** resolved

- [x] Provide one development check, reused by CI, that exports contracts, regenerates both SDK families, builds/verifies the outputs, and fails on differences from committed artifacts.
- [x] Verify actual mounted methods and paths are covered by canonical documents. Reconcile the migration inventory and inspect remaining outbound call sites so no project-owned API operation remains undocumented or handwritten.
- [x] Verify repeated export/generation is deterministic and failed generation leaves prior artifacts intact. Exercise built JavaScript package exports and real Rust consumers, not only generated source inspection.
- [x] Document export, generation, validation, API/topology document boundaries, and the rule against hand-editing generated artifacts. Remove remaining obsolete live-server commands and stale guidance.
- [x] Reuse existing scripts and test boundaries; do not build a separate audit framework or add per-service coordination machinery.
- [x] Run formatting, clippy, focused contract/integration checks, cargo hack test --feature-powerset --all-targets, and just crap. Record commands, results, and unrelated failures or environment limits without claiming unrun validation passed.
- [x] Confirm local/offline behavior, authentication and transport safeguards, and Setup/Node-join authorization remain intact. Contract tests alone do not claim full native GUI, production deployment, or cross-platform acceptance.


## Answer

Implemented on `implement/openapi-enforcement` in the isolated `of-openapi-enforcement` worktree, based on integration `ef6441d74`. No parent checkout edits. The approved seams were existing SDK/actual-router listeners and developer commands. Complexity is low (extend the existing qualification runner, no audit framework); risk is medium because CI must catch omissions and clean-checkout failures without implying authorization or deployment acceptance; priority is high for preventing contract drift.

### Deliverable

- `pnpm openapi:check` is the single developer/CI gate. It exports all ten documents twice without state/deployment and checks byte equality with committed contracts; bootstraps package declarations; regenerates both SDK families twice with complete-tree comparisons; rejects HEAD/staged/untracked artifact drift; builds ESM/declarations and real Rust owner/Unified consumers; and runs existing generation-failure, SDK, production-registration, fresh-host, Setup, transport and actual-owner/Node-join qualification.
- Added public built-package export tests for all 11 JavaScript entry points. Red: a clean checkout reports missing built exports. Green: the shared command builds prerequisites and all exports load. The clean-worktree gate also exposed Storage staged compilation needing built IdP declarations; the command now supplies that prerequisite. Generation itself retains staged validation/publication and rollback, with no new adapter.
- CI adds an independent 45-minute OpenAPI job with Node 26, pnpm 12.4.1, Java 21, stable Rust/rustfmt and native DBus build prerequisites, read-only permissions, no persisted checkout credentials or deployment secrets. It runs the same command, not a CI-only variant. Workflow syntax guidance was checked against the github-actions skill reference `references/workflows-and-actions/workflow-syntax.md`.
- Reconciled all 128 operations and inspected remaining Rust HTTP sends/browser-native fetches against production registration, explicit Setup/history/Storage/Node-join composition and the existing actual-router tests. Remaining raw operations are the Setup framework OPTIONS TLS probe, fixtures, arbitrary fetch/external OIDC and non-HTTP messages, not handwritten project-owned fallbacks. Inventory retains concrete boundaries and evidence.
- Updated API workflow/topology/prerequisites, package README, inventory and stale README/security/operations guidance. Removed the obsolete live `OPENAPI_SPEC_URL` command and browser-handoff exclusion claim. Contracts/templates are authoritative inputs; derived contracts and either SDK must never be hand-edited. No runtime/authentication policy was changed.

### Fresh validation

Commands ran from this worktree via Python subprocess cwd, with cached Cargo artifacts reused from the ticket-02 worktree using `CARGO_TARGET_DIR` and `CARGO_NET_OFFLINE=true`. Dependencies installed with `pnpm install --offline --frozen-lockfile --ignore-scripts` (419 reused, zero downloaded). This is no-running-deployment/cached-offline evidence, not a network-isolation claim; integration tests use controlled loopback listeners.

- `pnpm openapi:check`: **passed**, including two identical exports, two identical full TypeScript/Rust regenerations, generated compilation, built ESM/declarations, real Rust consumer check, 42 JavaScript tests (including six failure-preservation cases and 11 export loads) and 16 Rust tests. One legacy explicit Setup exporter test is intentionally ignored because the canonical exporter supersedes that opt-in fixture. Rust counts: three owner-registration comparisons, one independent handoff contract, six Setup/fresh-host contract checks, five generated transport/actual-owner checks, one built host-composed Node-join check.
- Deliberately changed a canonical contract title and ran the shared command: **rejected** with the committed-contract mismatch. Restored exact original bytes. Deliberately changed generated IdP source and ran the shared command: **rejected** with SDK-regeneration drift. Restored bytes; no derived artifact change remains.
- `node --test scripts/generated-package-exports.test.mjs`: **11 passed** independently after build.
- `cargo fmt --all -- --check` and `cargo clippy --workspace --lib -- -D warnings`: **passed**.
- `python3 scripts/check_server_dependencies.py`: **passed**; no server-to-server dependency edges.
- `pnpm exec biome format scripts/qualify-openapi-clients.mjs scripts/generated-package-exports.test.mjs` and `git diff --check`: **passed**.
- Workflow parsed as YAML and checked for the exact shared command, timeout and read-only permissions. GitHub-hosted execution was **not run** locally; no hosted CI acceptance is claimed.

### Bounded validation limits and reused evidence

- `cargo hack test --feature-powerset --all-targets` was attempted with a **180-second bound**. Configuration 1 passed; configuration 2 was the expected no-target no-op; the run timed out compiling configuration 3 of 10. No fresh complete powerset pass is claimed. Ticket 02 at `ef6441d74` already records all ten root configurations passing; this ticket changes no Rust implementation and does not unnecessarily repeat that expensive completed work. Neither result is a workspace-wide powerset claim.
- `just crap` was attempted with a **180-second bound**, then its process group was terminated during coverage compilation. No CRAP score/coverage result is claimed.
- Did not repeat the full Unified suite that already exceeded **1,100 seconds** in ticket 02. Existing signer-fixture batch failures, all-target test Clippy lints and Password Manager localization/declaration issues remain as recorded there; library Clippy is clean here. No unrelated fixture/lint fixes or threshold/deadline relaxations were made.
- Ticket 02 evidence is retained (not claimed freshly rerun): actual independent HTTPS Create/Join and source-offline reopen, local/current owner authorization, renewal and service/domain validation, native lifecycle/revocation, browser app/package tests. The fresh shared gate additionally confirms unchanged Setup security contracts, native-root TLS rejection, redirect refusal, streaming/request/deadline bounds and real unauthorized owner calls. Decoding/schema publication grants no authority.

No full native GUI, production deployment, real OS-provider installation or cross-platform acceptance is implied.
