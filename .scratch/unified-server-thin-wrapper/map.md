# Unified Server: Thin Wrapper Refactor

This refactor aims to strip `unified-server` of all "Independent" (local-only) logic, making it a pure composition layer.

## Domain Context

- **Independent Path**: The logic used by the CLI to perform node bootstrapping, provisioning, and joining. This now lives in `idp-server`; `unified-server` supplies concrete host capabilities.
- **Hosted Path**: The logic used when a node joins an existing, running `unified-server` via its HTTP API.

## Module Mapping

| Current Module (`unified-server`) | Target Module (`idp-server`)                 | Purpose                         |
| :-------------------------------- | :------------------------------------------- | :------------------------------ |
| `local_join.rs`                   | `idp-server/src/local_join.rs`               | Node joining state machine      |
| `local_provisioning.rs`           | `idp-server/src/local_provisioning.rs`       | Node provisioning logic         |
| `bootstrap_config.rs`             | `idp-server/src/bootstrap_config.rs`         | Bootstrapping configuration     |
| `bootstrap_verification.rs`       | Retained unified runtime adapter             | Composed HTTPS qualification    |
| `credential_bootstrap.rs`         | `idp-server/src/credential_bootstrap.rs`     | Credential generation           |
| `issuer_discovery.rs`             | `idp-server/src/issuer_discovery.rs`         | Identity issuer discovery       |
| `local_tls.rs`                    | Existing `api::LocalTls`, re-exported by IdP | TLS handling; IdP owns settings |

## Success Criteria

- `unified-server` contains no "local" or "independent" logic modules.
- `unified-server` CLI remains fully functional by consuming the relocated modules from `idp-server`.
- `unified-server` is a pure composition of `idp-server`, `management-server`, and `storage-server`.

## Notes

- Tickets 01/02 are compilation-coupled; 01 claimed first. Existing public composition, hosted setup, and local workflow tests remain the regression seams; no new test seam is introduced.
- Complexity/risk: medium/high (durable identity and secrets); preserve journal formats, operation order, runtime composition, and coverage. No feature powerset until ticket 03.

## Decisions-so-far

- [01 resolved](issues/01-relocate-core-logic.md): IdP owns bootstrap settings, durable host/credential/reset state and local Create/Join; generic host capabilities keep concrete endpoint/runtime/security composition in unified. TLS reuses `api`; HTTPS qualification stays a composed-runtime adapter.
- [02 resolved](issues/02-update-cli-to-consume-idp-server.md): CLI consumes IdP workflows through `UnifiedBootstrapHost`; concrete unified aliases preserve host call sites. `UnifiedConfig` preserves concrete IdP, management, and storage configurations; bootstrap settings are projected and applied through the host adapter.
- Reviewed moved modules against `8667a29c3`: only host abstraction, imports/visibility, configuration ownership, and isolated test preset wiring change; retained journal shapes, authorization checks, and operation order do not.
- Focused validation via temporary manifest mirror: CLI-enabled IdP/unified all-target check and Clippy pass with warnings; Rustfmt/diff checks pass. Bootstrap settings 3/3, router 1/1, composition integration 2/2, hosted setup 3/3 plus isolated child, credentials 3/3, setup-runtime 1/1. Local provisioning 8 pass/2 baseline failures; HTTPS 7 pass/1 baseline timeout/1 child ignored by design.
- [03 resolved](issues/03-purge-unified-server.md): removed obsolete root `independent_*` modules; preserved concrete multi-owner regression coverage under `owner_composition_tests`, with corrected subprocess paths. Architectural violation FIXED through minimal owner settings and host provisioning capabilities, preserving concrete server configs and nondefault options. The unchanged dependency checker passes; IdP has no management/storage server references.
- Cleanup focused validation: config 5/5, bootstrap 3/3, composition integration 2/2, minimal IdP settings 1/1; owner-composition listing compiles and discovers 14 tests. Final Rust sources match the validated mirror; rustfmt and diff hygiene pass. Existing failed assertions remain intact.
- `.validation` is temporary, untracked, and excluded from the commit. It mirrors missing sibling path dependencies without modifying their repositories and remains for parent cleanup. Broad tests, final powerset, and CRAP were not repeated per handoff and remain unqualified.

## Fog

- Baseline-reproduced local failures: existing CLI-host denial compares changing redb bytes; stopped-host recovery cancels startup before serving completes. Setup HTTPS expects 404 but receives 415. HTTPS Create/Join approval times out on both original and extracted code. These assertions were not weakened or ignored.
- Runtime composition acceptance needs built `packages/management-client/esm/index.js`; baseline and extracted runs cannot qualify that JavaScript consumer. The mirror now supplies scripts, but SDK output is absent. CLI help first compilation timed out at 180 seconds, then the bounded retry passed. IdP library first compilation also timed out at 180 seconds, then completed 53 pass/1 missing-private-signer failure/5 ignored. That IdP fixture is documented as a historical baseline failure in `docs/public-persistence.md`, not independently re-baselined here.
- A broader local filter accidentally included multi-full-node tests: two failed before the 180-second bound; no complete suite summary or baseline classification. No broad/full-suite success is claimed.
- The former `idp-server -> management-server/storage-server` architectural violation is FIXED in ticket 03, not an outstanding follow-up; dependency policy was not weakened.
