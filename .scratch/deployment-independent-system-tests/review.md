# Single-implementer review — private system-test roots

## Scope and finding

Review branch `refactor/system-tests-review`, worktree
`/home/nathan/code/rust/aicacia/of-system-tests-review`, based on integration
`095f78754759efef9658d6955f2ffc83a3d501ac`.
Read the feature spec and coverage inventory; loaded `tdd` and `ponytail`.
Only the concrete P2 finding is fixed; the spec and acceptance criteria are unchanged.

**P2 — `tests/system/create.rs:98` at the reviewed base:** `Node::new()` used
`tempfile::tempdir()`, whose Unix directory mode is `0777 & ~umask`. With a
permissive umask, the root containing durable mock-provider snapshots, executable
trust-tool substitutions and bounded service logs was accessible to group/other.
Mode-0600 provider files alone did not protect the containing directory or logs.

## Fix and regression

- Create the shared fixture root with `tempfile::Builder` and explicit Unix
  `fs::Permissions::from_mode(0o700)`. Protection is requested at creation, not
  applied afterward; existing imports suffice. All Create, restart, Join and
  denial callers continue to use the same constructor.
- Add the non-ignored runnable `create::root_is_private` test. A child shell sets
  umask `000` and execs the exact test executable/test; the child constructs a Node
  and asserts that filesystem metadata has zero group/other permission bits.
  The parent test runner never changes umask or process-global environment.
  The permissive child umask makes the check independent of the inherited umask
  and exposes the original bug even when the caller normally uses `077`.
- TDD red: before the fix, the child assertion observed `63` (`0o077`) instead
  of zero, and the parent reported child failure. The empty diagnostic fixture
  retained by that intentional failure was removed afterward.
- TDD green: the identical regression passed after the Builder change.

Complexity/risk: small fixture-only security fix, no dependencies or production
behavior changes. A real-process Create execution checks that service launch and
ordinary owner/resource use still work under the restricted root.

## Validation

Commands ran from `/home/nathan/code/rust/aicacia/of`. All heavy Cargo commands
ran serially with a 600-second tool bound per invocation/group; none timed out.
For each Cargo command below, the common options are:

```text
--manifest-path ../of-system-tests-review/Cargo.toml
--target-dir /home/nathan/code/rust/aicacia/of/target
-j8
```

| Command (common options after subcommand) | Result |
| --- | --- |
| `cargo test -p idp-unified --test system_create create::root_is_private -- --exact --nocapture` before fix | Expected FAIL: group/other bits `0o077`; compile 2m37s, test 0.13s. |
| Same regression after fix | PASS: one parent and its exact child test, no service prerequisites; compile 7.34s, test 0.01s. |
| `cargo build -p idp-unified --example system-test-service -p idp-server --bin identity-database` | PASS, 2m19s. |
| `cargo test -p idp-unified --test system_create unified_create_and_use -- --exact --ignored --test-threads=1 --nocapture` | PASS: one actual-process scenario, 22.28s (Cargo preparation 27.99s). Fresh Create, public readiness/login and ordinary authorized owner/resource operations. |
| `cargo clippy -p idp-unified --test system_create -- -D warnings` | FAIL on existing dependency/generated-client lints, including 93 errors in `of-idp-client` (`empty_docs`, `derivable_impls`). Unrelated code left untouched. |
| `cargo clippy -p idp-unified --test system_create --no-deps -- -D warnings` | PASS, 15.79s: strict linting scoped to the affected target. |
| `cargo check -p idp-unified --all-targets` | PASS, 15.80s. |

Also passed `cargo fmt --manifest-path ../of-system-tests-review/Cargo.toml
--package idp-unified -- --check` and
`git -C ../of-system-tests-review diff --check`.
No generated contracts, dependency boundaries or production APIs changed.
No CRAP, feature-powerset, broad-suite or additional deployment certification run.

## Retained coverage gaps — not full parity

The limitations in `coverage.md:399–425` remain real and unchanged:

- Applicable **live post-Join** reconciliation and learned denial remain in the
  retained Unified/private `full_node_tests` cases. Their runtime endpoint and
  owner-database synchronization boundary has no supported public actual-process
  trigger. Shared initial-Join history denial and local User deletion are not
  substitutes. Cross-layout actual-process live reconciliation remains a concrete
  qualification/migration gap, not implemented by this fix. Ticket05 recorded two
  retained tests passing with a 64 MiB stack; this review did not rerun them or
  the third retained partition/reopen case.
- Mixed owner/permission/residency/copy-cleanup fixtures still retain private
  authorizers, injected owners and scheduling gates; physical owner-local
  relocation and migration of remaining ordinary legacy HTTP operations are
  deferred, not fully implemented. Existing bodies remain enabled. This patch
  adds neither a public hook nor a replacement private-process shortcut.

The root security regression and one representative Unified process scenario do
**not** establish full deployment parity, completion of deferred fixture work,
independent-layout live synchronization, real OS provider/trust qualification,
or platform/network certification. No spec rewriting or waiver was made.

## Integration and ownership

`GIT_EDITOR=true git -C ../of-system-tests-review merge --no-edit
refactor/deployment-independent-system-tests` returned **Already up to date**;
integration tip was `095f78754759efef9658d6955f2ffc83a3d501ac`.
Only `tests/system/create.rs` and this report are owned/staged for this fix.
The main checkout’s eight unrelated staged AD setup-simplification entries are
untouched. No merge into main/integration, push, or unrelated cleanup performed.
