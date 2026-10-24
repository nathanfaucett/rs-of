# Ticket02 implementation report

## Base and ownership

- Worktree: `/home/nathan/code/rust/aicacia/of-system-tests-02`.
- Branch: `refactor/system-tests-02`.
- Confirmed starting HEAD and integration tip:
  `191962716f3edd7fd0c0ffbd8e8ec6c008511393`.
- Read the full spec, ticket02 and AGENTS; loaded `tdd` and used the already
  approved generated-client/protocol seam. No production test hooks or server
  implementation changes. No existing Unified test files edited.

## Implementation and coverage

| Location | Responsibility |
| --- | --- |
| `tests/system_create.rs` | Explicitly opt-in, layout-named executions of one shared scenario |
| `tests/system/create.rs` | Concrete process fixtures, public setup/readiness correlation, shared login/discovery/owner health/resource/anonymous-denial assertions |
| `examples/system-test-service.rs` | Existing mock-provider substitution followed by production CLI `run`; separate OS process per service, no substitute servers |
| `tests/system/README.md` | Runnable commands, exact supported topology, safety and qualification limits |
| Root manifest/lock | Test-only existing workspace dependencies; no new production crate |

Unified admits a fresh Create through its public generated setup client and
protected code, polls current public readiness, and verifies the exact completed
Create to obtain the public operator login Client. Independent IdP/Device/Storage
use three distinct capabilities and the supported correlated tuple; only IdP gets
the password. Each worker must be currently ready and publish the exact matching
nonce, purpose, administrator, Node, issuer and all service bases. Nonempty public
endpoint evidence must match across all three owners.

Independent runs the actual `identity-database` binary and production IdP,
Device, Storage and Management CLIs in five processes. Identity SQL lives outside
all consumer directories; Device owns its separate local Device database.
Management receives an ordinarily registered Application/Client and the explicit
identity SQL connection, not a fourth setup request. The common independent
setup contract is consumed using the generated IdP SDK at each worker's actual
base. Ordinary operations consume each generated owner SDK. No hand-written
HTTP, direct domain-store reads, row seeding or ready-marker injection.

Shared assertions discover the Canonical Issuer, log in ordinarily as the Initial
Administrator, read User info, observe Device and Management database health,
create/read the same Storage filesystem through the generated Storage SDK, and
require anonymous access to that resource to fail with 401/403.

## Coordination with ticket01

No migration/removal edits were necessary. Preserve the existing historical
`hosted_setup_integration_tests::production_api_create_authentication_and_offline_reopen`
and `local_provisioning_tests::https_bootstrap_tests` coverage while migrating it:
this ticket replaces only the common fresh Create/login/resource slice. Their
malformed-wire, exact retry, cancellation, interrupted/retained-state, Join,
offline reopen and private recovery assertions are **not** replaced by ticket02.
The common independent setup SDK is deliberately generated, not handwritten;
per-owner SDK drift beyond the shared worker contract remains existing contract
qualification's responsibility. No OpenAPI documents or generated SDK output
needed changing to execute the supported topology.

## Focused validation

All Cargo commands below used
`--target-dir /home/nathan/code/rust/aicacia/of/target` in this worktree (shared
cache, Cargo locking respected); heavy Cargo commands were run serially, with
`-j 8` on final checks/builds/tests. No unrelated job was killed.

- `cargo build -p idp-unified --example system-test-service -p idp-server --bin identity-database`: passed; final rebuild 16.55s.
- `cargo test -p idp-unified --test system_create -- --ignored --test-threads=1 --nocapture`: **2 passed**, 50.39s; repeated **2 passed**, 52.32s. Both layouts ran, none skipped in those invocations.
- `cargo check -p idp-unified --all-targets`: passed (default/CLI root targets, including the test and launcher).
- `cargo check -p unified-server`: passed (default library, without enabling its CLI feature).
- `python3 scripts/check_server_dependencies.py`: passed, no direct server-to-server edges.
- `cargo fmt --package idp-unified --check`: passed.
- `cargo clippy -p idp-unified --test system_create --example system-test-service --no-deps -- -D warnings`: final check passed without warnings (7.92s). The first non-strict check identified one collapsible-if warning, subsequently fixed.

Earlier evidence is not erased: the first red-test attempt stopped in dependency
compilation (`semver` missing rlib for a build script), not at its intentional
assertion. A first `-j 2` check and two prerequisite builds exceeded 200s each;
subsequent bounded commands completed. Runtime red results exposed incorrect
fixture assumptions: unsupported arbitrary owner setup prefixes, independent
Create's issuer=IdP-base rule, nullable generated peer representation, missing
ordinary Application registration before Client registration, and Storage's
fixed `/storage` resource routes. These were fixture corrections, not production
contract changes. A proposed cross-application RBAC list was removed because the
Initial Administrator's identity permissions do not imply arbitrary Application
role-read permissions; the required authorized resource assertion was retained.

With production-cost independent password configuration, one serial paired run
failed the independent resource POST with HTTP 503 after both layouts had also
passed individually. No mutation retry or deadline increase was added. The
independent fixture now uses the existing test convention of 1 MiB/one-iteration
password hashing; two final paired runs passed. The earlier 503's exact cause was
not established; do not infer production-cost availability/performance from the
small-fixture passes. Child authentication timing diagnostics are enabled and
bounded. Full-cost/default-stack/load qualification remains unclaimed.

## Limits / remaining work

- No CRAP, coverage, powerset, full-workspace suite, platform matrix, GUI, real OS
  provider/trust installation, WAN/container or production deployment certification.
- Mock-provider state is per-process memory. This fixture does not qualify
  retained restart, Join, source-offline reopen, recovery or synchronization.
- IdP/Device/Storage use verified HTTPS with isolated native CA trust. Management
  and identity SQL use their supported literal-loopback HTTP/plaintext listeners;
  no external TLS proxy or application mTLS is qualified.
- Fresh neutral host TLS/admission preparation uses the public host API only;
  installation/domain setup itself is entirely via generated HTTP clients.
- Owned children are killed/reaped in reverse start order on success or unwind;
  this is teardown safety, not graceful-shutdown behavior qualification.
- Failure roots remain private for inspection; successful runs remove their roots.
- The shared Cargo cache and sibling path dependencies reflect the local checkouts;
  unrelated sibling `ofdb` lock-edge churn is excluded from this ticket's diff.
