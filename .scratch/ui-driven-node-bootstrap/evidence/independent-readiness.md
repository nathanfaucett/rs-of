# Independent current readiness, retained default listeners and Join record cleanup

## Source and execution boundary

Worktree `of/.scratch/worktrees/bootstrap-readiness`, branch `implement/bootstrap-readiness`, base `68129cb00` (completed native qualification). This slice owns retained independent CLI listener selection, current independent setup observation, Create-only durable ID cleanup, corresponding production-seam regressions and phase-cost observations. Transfer completion and explicit reset are separate agents' work. No ticket is closed.

**Rust execution is pending the parent's build token. No Cargo build, check, test or Clippy command has been run for this slice. There is no observed red/green pass and no measured phase-cost result yet.** Regression source was added at the existing setup API/configuration seams; pending execution is not acceptance. No scenario deadline, production HTTP timeout or stack limit was increased. No real OS trust store, profile or protected provider was modified.

Complexity/risk/priority: high-priority lifecycle/security corrections. Reuse the existing control, owner handles, normal policy contract and test logger. The main risks are false current readiness, a callback/host reference cycle retaining stores, replacing invalid state, remote readiness becoming a source-peer startup dependency, and accidental removal of normal authorization.

## Implementation

- Management/Storage default listener selection recognizes retained `<domain>.setup-host` state even after the domain directory exists. Fresh and retained setup use 3001/3002 respectively; explicit ports remain authoritative. An existing domain without setup-host state keeps the historical configured/default listener rather than being claimed as fresh setup. IdP's default 3000 selection does not have the domain-existence conditional.
- `IndependentCreateControl::set_readiness` accepts `Fn(IndependentCreateControl) -> Future<Output = io::Result<()>> + Send + Sync + 'static` (the future is `Send + 'static`). The callback is memory-only. The host passes the control to each invocation, rather than the callback capturing a strong reference to its own host. It runs outside the lifecycle mutex and readiness lock. Normal routers are not disabled when observation fails.
- A completed status rechecks its callback. Failure returns HTTP 503 with the existing nonsecret status shape, `state:"unavailable", ready:false`; successful subsequent checks recover observation without rewriting setup or reseeding. Admission corruption still refuses. Retirement remains a setup capability transition, not normal API enablement.
- IdP checks exact retained worker evidence, current protected consumer material, its current approved signer and private signer availability, active setup User, public login Client and live confidential registration/root metadata through its existing owner. No remote token/health/relationship qualification is performed.
- Management checks exact retained worker evidence, protected consumer/endpoint material, its current approved Device/endpoint identity, current owner/policy agreement, administrator allowance and pending-cleanup fence through the already-open owner. Before completion the seed owner must still match exactly; completed readiness follows the Device's current owner, not an immutable original-owner grant.
- Storage checks exact retained record/registration receipts, current local service material and existing catalog roots. Its status callback uses the existing current Management policy read for the configured endpoint. This is the required policy invariant, not a full HTTP qualification suite. Reconstructing completed workers remains existing-only and does not call a source peer or perform policy HTTP qualification; policy observation needs the Node's configured Management/IdP services, not a remote Installation source.

## Reset-agent handoff: exact durable record changes

No migration or compatibility wrapper is supplied. Ordinary IdP/Management opens reject version-1 records unchanged; explicit reset must still preserve exact historical cleanup references.

IdP `worker.json` is now **version 2**. Fields otherwise remain `user`, `login`, `member`, `key`, `client`, `secret_attempted`, `domain_attempted`, `inventory_attempted`, `endpoint`, `registrations`, `request`, `approval`, `imported`, `complete`. `credential` is an optional UUID serialized **only on Create**; Join rejects its presence and does not generate it. The existing `user`/protected inventory association is not rewritten by this change: reset must use the actual protected inventory's owner binding where required, not guess from a completed Join's approved User.

Management `worker.json` is now **version 2**. Common fields remain `client`, `device`, `user`, `evaluator`, `secret_attempted`, `endpoint_attempted`, `endpoint`, `domain_attempted`, `source`, `approval`, `imported`, `complete`. Top-level `role` and `permissions` are removed. Create alone has `create: { "role": <UUID>, "permissions": [<UUID>, ...] }`; Join omits `create` and rejects fresh grant IDs. This object contains no protected keys and creates no new provider slots.

Storage stays **version 1**, unchanged: `clients`, `attempted`, `endpoint`, `domain_attempted`, `complete`.

All provider namespaces stay `independent-setup:<absolute setup-host directory>`. IdP owns its `client` slot; Management owns its `client` and `endpoint` slots; Storage owns both `clients` slots. IdP Node private material and inventories stay with the existing IdP owner. `initialized.json`, admission intent/retirement, registration receipts and individual attempt fences remain intact. Readiness adds no reset manifest or durable progress fact.

## Regression source (not executed)

- Both owner startup suites: ordinary port-unspecified configuration before setup, retained pre-domain retry, post-domain retry/completed reopen and explicit-port precedence. Existing Management existing-domain/no-admission regression remains.
- Production independent HTTPS Create fixture: transient signer-provider refusal; missing consumer protected material on each owner; readiness recovery; unchanged durable evidence/no regeneration; ordinary unauthenticated protected routes still deny.
- The same fixture arranges extra Devices and explicit test permissions only while owners are stopped, using public Management persistence/service APIs (no IdP Device write or signer enrollment). It then uses real verified HTTPS/scoped OAuth to check signer-free approved policy evidence and unknown/pending/revoked/mismatched-policy-owner denial. Ordinary User-authorized revocation proves learned denial with the existing HTTP client/token and after stopped offline reopen. Live exact restriction changes make Management/Storage status and authenticated Storage operations deny/recover. Ordinary IdP-authorized signer revocation makes IdP readiness fail.
- Production independent Join fixture checks version-2 records omit IdP `credential` and Management `create`/old top-level grant IDs. Existing source-offline completion/finalization/reopen remains the seam.

These tests do not certify transport DATA for the additional signer-free endpoint, rendered GUI, real provider/trust installation, WAN/platform/container deployments, independent transfer acknowledgement or explicit reset.

## Phase observations (execution pending)

Simple `Instant` measurements surround existing concrete worker calls; the existing `api_create_tests::SetupTimingLog` is reused by isolated independent children. Messages contain owner/purpose/phase/elapsed milliseconds only, not usernames, passwords, tokens, codes, peer offers or keys. No new framework, dependency, deadline or SLA is introduced.

Create phases distinguish consumer/provider keys, schema/open, protected inventory, password credential, signing-key/root seed, Client registration seed, Management semantic seed, runtime serving and relationship registration/wait. Join separately identifies discovery, approval wait, protected signer work, schema/open, approved public-state import, local Client registration and serving. Storage identifies consumer keys, relationship wait and catalog-open/serving. Owner contracts that combine key production and semantic persistence are labelled as such rather than falsely attributed to KDF-only work.

Actual values, build/profile costs and external trust/provider costs must be filled from authorized runs; old unified or independent scenario totals must not be substituted.

## Commands

Run from this worktree **only after the parent grants the sole Rust build token**, sequentially with a 200000ms tool bound and unchanged scenario bounds:

```sh
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p management-server -p storage-server -p unified-server --lib startup::tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib production_independent --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p api --lib independent -- --nocapture --test-threads=1
```

Executed static validation: scoped `rustfmt --edition 2024 --config skip_children=true --check` on the eleven changed Rust files, and `git --no-pager diff --check`, both exit 0. Final checks, source provenance and measured results remain pending build authorization. No broad feature powersets/CRAP or privileged installation commands were run.
