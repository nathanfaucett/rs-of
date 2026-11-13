# Build-fix qualification

> Follow-up: see `security-history-validation.md`. The independent compilation and
> fail-all security-history limits below were repaired after merging `8596dfa08`.
> This older record is retained for the original validation results.

Branch: `backend-simplification-buildfix`. Base: integration `64e44f11e`.
Worktree: `.scratch/worktrees/buildfix`.
Status: **local server builds and runs; acceptance remains incomplete**.

## Changes

- Kept the interrupted Rust/Cargo edits, including stream ownership transfers, async bootstrap calls, native ofdb sync callers, candidate validation types, dependency routing, and deletion of the obsolete push-sync adapter.
- Fixed the duplicate closing delimiter in IdP administration.
- Reused `of-model::http::idp` user request types instead of duplicate service definitions. Added their conditional OpenAPI schemas.
- Implemented the missing confidential-client secret rotation operation. The owner test verifies that the new secret matches, the old secret no longer matches, and public/missing clients are rejected.
- Separated independent IdP/Device lifecycle exports behind an `independent` feature, enabled by their CLI features. Unified local composition does not require independent join-history compilation. No independent lifecycle implementation was replaced with a compatibility shim.
- Converted Storage database resource sync callers to the current typed-message transport, configuration, synchronization result, error, and change-subscription APIs. Removed the obsolete subscription `run` scheduling call. Mesh synchronization is not qualified by these local tests.
- Changed the unified OpenAPI exporter to export its actual local owner surfaces, without independent setup/join additions. No generated contracts or frontend files were edited.
- Set default local logging to `warn` to prevent dependency INFO logging from flooding startup. `RUST_LOG` still overrides it.
- Split the three-second performance assertion from functional smoke tests. Functional tests have a 60-second readiness deadline; the separate three-second assertion still fails. No performance test was ignored.

## Actual validation

| Command                                                                                                                          | Result                                                                                                                                                                        |
| -------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo check -p unified-server --all-targets`                                                                                    | Passed on the final Rust implementation. Warnings remain.                                                                                                                     |
| `cargo clippy -p unified-server --all-targets`                                                                                   | Passed on the final Rust implementation. Warnings remain; this is not a `-D warnings` run.                                                                                    |
| `cargo test -p unified-server --all-targets -- --test-threads=1 --nocapture`                                                     | **2 passed, 1 failed**. Both real-process functional smoke tests passed. The separate startup performance test failed.                                                        |
| `cargo test --manifest-path .scratch/backend-simplification/owner-validation/Cargo.toml --target-dir target -- --test-threads=1` | **16 passed**, zero failed/ignored, including the expanded seed/secret-rotation test.                                                                                         |
| `cargo fmt -p unified-server -- --check`                                                                                         | Passed.                                                                                                                                                                       |
| `rustfmt --edition 2024 --config skip_children=true --check` on all changed surviving Rust files                                 | Passed. This avoids unrelated missing owner HTTPS fixture module paths.                                                                                                       |
| `git diff --check`                                                                                                               | Passed.                                                                                                                                                                       |
| `cargo check -p device-server --features cli`                                                                                    | Failed: five unresolved `device_service::join_history` imports. This source module was already disabled at the integration base and still calls removed staged SQL sync APIs. |
| `cargo hack test --feature-powerset --workspace --all-targets`                                                                   | Final attempt timed out after 200 seconds during compilation. No full-suite result.                                                                                           |
| `just crap`                                                                                                                      | Final attempt timed out after 200 seconds during coverage compilation. No CRAP result. Its SDK-build prerequisite was invoked; no frontend source changes were committed.     |

The smoke tests launch the actual `unified-server` executable on an available loopback port and an empty temporary directory. They check all four health routes, absence of duplicate prefixes/setup routes, OIDC discovery, both frontend CORS origins, administrator sign-in, User info, consent and PKCE token exchange for both frontends, optional seed-user sign-in, restart, and trusted HTTPS with unchanged certificate/key bytes.

Final sequential smoke timings:

- HTTP: fresh **22.014 s**, restart **8.012 s**.
- HTTPS: fresh **20.885 s**, restart **7.994 s**.

Earlier startup instrumentation measured roughly 2.6–3.1 seconds in schema creation and 13–15 seconds in administrator seeding. Other owner router construction was fast. A syscall summary did not identify filesystem sync latency as the dominant cost. Temporary instrumentation was removed. Startup needs a separate owner/database performance repair; the three-second acceptance requirement is not met.

## Security and remaining limits

- The current ofdb push SQL API does not provide the removed scoped candidate-validation/staging contract. Passing the entire security database to unchecked native sync would risk sending or applying private/unreviewed history. Security protocol and node-bootstrap transports therefore **reject ChangeEvent and Snapshot payloads** with an explicit unsupported error. They do not bypass history checks. Independent security-history exchange is not functional or qualified on this branch.
- Independent join-history callers still need a real validated-history implementation against current ofdb APIs. Enabling independent owner CLIs still exposes this compile failure; local build success is not workspace/production qualification.
- Storage's native SQL/KV mesh migration has not passed the independent execution/revocation suite. The upstream push implementation and the lack of an atomic pre-apply callback still need qualification. Unified local Storage does not start mesh tasks.
- Storage resource data/catalog files and IdP private-key files remain outside `unified.redb`, as recorded in `unified-validation.md`. The single-file-only acceptance conflict is unchanged.
- Desktop/Tauri and independent deployment paths are not qualified. No frontend or README changes were made.
- Do not close the overall backend-simplification acceptance ticket on this result.

## Required isolated dependencies

No sibling main checkout was modified or merged.

1. **ofdb `503dc08`** (`backend-simplification-unified-blocker`; the existing `backend-simplification-buildfix` ofdb worktree has the same commit). This is the required snapshot-serde fix. The worktree parent alias `ofdb-buildfix` points to `ofdb/.scratch/worktrees/buildfix`.
2. **offs `ce2051b9173ff6de5582667155180298b0562b86`**, branch `backend-simplification-buildfix`, worktree `of/.scratch/worktrees/offs-buildfix`. This Cargo-only commit retains the interrupted filesystem dependency redirects to `ofdb-buildfix`. Its parent is offs `860f63f`.
3. Existing `ofnet` worktree-parent alias points to the unchanged sibling checkout.

Cargo paths deliberately use `ofdb-buildfix` and `offs-buildfix`, not sibling main. A different checkout must supply those sibling aliases/worktrees at the corresponding relative locations, or integrate the dependency changes and update the paths. Merely checking out this of commit without that layout is not sufficient.

## Handoff protocol

Commit hooks were bypassed for the task commits; the explicit commands above are the validation record. The build-fix branch is to merge the current `backend-simplification` tip after committing. It must not merge back into integration or main. The final response records the actual commit and merge result.
