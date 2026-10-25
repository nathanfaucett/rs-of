# Ticket 05: visible startup and trust evidence

## Scope and ownership

- Worktree `of-setup-05`, branch `setup-simplification-05`, started at integration `23e9006ca`. Claimed ticket before implementation; read spec, AGENTS, tracker, ticket 01 and its evidence.
- Complexity/risk/priority: medium asynchronous coordination complexity, high trust/lifecycle risk, high priority. One lifecycle owner and cancellation token; no additional persisted workflow, dependencies, Create/Join implementation, completion removal or maintenance UI rewrite.
- Used TDD and both Svelte skills at the spec-authorized mounted-page/native-injection/protected-provider seams; no interview or real OS trust/profile changes. All tests use isolated roots. The owned-process check executes only a harmless injected `sleep` process, never a trust installer.
- The requested worktree is outside the editor's registered roots. Worktree-only shell paths were used; no main-worktree source edits, branch switches or commits.

## Implementation

The native window is constructed before startup is spawned. Blocking retained-host preparation is offloaded. Observation does not wait on the lifecycle operation lock while startup is pending: no published host projects `starting`, and the mounted setup page polls with navigation cleanup.

The retained HTTPS listener is probed with native-root OPTIONS (200/405), independently of readiness-bearing GET. Only a typed rustls `InvalidCertificate(UnknownIssuer)`, including nested IO sources, reaches necessary-trust consent. Already-trusted launches never invoke installation. Bind, HTTP, other certificate and retained-state failures cannot authorize trust changes or new identity. No marker waives verification. After explicit consent, the same retained identity is reopened/reverified, installation is conditional on current need, the client is rebuilt and HTTPS is verified again. Post-install failure never becomes success or implicit repeated elevation. Production adds no custom root or plaintext fallback.

The page explains the certificate-authority anchor, possible elevation, impact on other OS-root consumers and lack of automatic removal before Allow/Not now. Refusal retains evidence; retry rechecks without remembered consent. Prerequisite/retry/repair categories and remote-target independence remain intact.

Start/retry, consent, reset/recovery and close use one lifecycle owner. Stop cancels before waiting for startup, installer ownership is joined/reaped, listener/worker cleanup precedes deletion, and close rejects late restart. Publication checks cancellation/closure under the same state lock as stop; cancelled hosts cannot populate the running slot. Observation does not present cancelled hosts as ready/running. Removed the obsolete synchronous startup-cleanup helper and block-on startup path. Native entry modules remain declaration-only/thin.

Stable native handoff, platform operations, observation actions and HTTP inventory document the changed contract. Create/completion (02), Join (03) and maintenance presentation (06) remain outside this slice.

## Red → green

Observed failures before the corresponding implementation:

1. Existing actual-TLS one-listener test supplied an installer that panics for already-trusted roots: old startup invoked it and failed. Verification-first startup passes without installation.
2. Unknown-root consent/retry test initially failed compilation because the consent error/category did not exist. After implementation it exposed nested IO error wrapping; walking typed IO sources correctly identifies UnknownIssuer. It now proves refusal cleanup, same CA/port retry and refreshed verification after injected installation.
3. Close-during-owned-installer test initially failed compilation because coordinated lifecycle ownership did not exist. It now proves close waits for owned startup, joins listeners and prohibits late restart.
4. Mounted setup test displayed Starting indefinitely instead of the trust explanation. Polling and consent integration make startup → explained consent → refusal → explicit retry → consent → awaiting-choice pass through visible controls.

Additional regression checks cover reset contention, rejected late publication, a harmless cancelled/reaped child process, unrelated occupied-port failure without installation, failed post-install verification, retained evidence loss, exact Origin/admission and existing reset safety.

## Final focused validation

Commands are relative to the ticket worktree (shell invocations used its explicit manifest/directory):

| Command | Result |
| --- | --- |
| `pnpm install --offline --frozen-lockfile`; `pnpm packages:build` | Passed; generated tracked SDK/browser/bin changes restored, not committed |
| `cargo test -p lidp --lib -- --skip native_shared_create_reopen_and_provider_safe_reset` | **22 passed**, one isolated-provider child ignored, one explicitly identified parent filtered |
| `cargo clippy -p lidp --all-targets --no-deps -- -D warnings` | Passed on final Rust |
| `pnpm exec vitest run --config vitest.residency.config.ts --maxWorkers=1` (app) | **15 passed**, including visible startup/consent/refusal/retry and existing mounted workflows |
| `pnpm exec vitest run --config vitest.auth.config.ts --maxWorkers=1 --no-file-parallelism` (app) | **181 passed**, seven files |
| Focused Installation/auth routing run | **68 passed** |
| Isolated owner-client run on changed code | **27 passed** |
| `pnpm build`; `pnpm check` (app) | Passed; **0 errors, 0 warnings** after SDK/localization generation |
| Svelte MCP autofixer on Installation page | No issues; timer/effect suggestions retained because polling owns cleanup and is not a pure derived value |
| `rustfmt --edition 2024 --check` on changed Rust; `git diff --check` | Passed |

DOM checks use the existing `RESIDENCY_JSDOM_MODULE` injection with jsdom installed only in disposable `.scratch/setup-simplification/test-tools`. No package/lockfile dependency change; the disposable directory was removed. Build/localization products are prerequisites, not source changes.

## Failures and limits

- Initial fresh native builds exceeded several 200-second bounds; subsequent compilation and focused tests completed. Initial Clippy compilation timed out; the resumed final changed-crate strict check passed.
- Full native test invocation failed the existing isolated provider Create/reopen parent: its child reaches `bounded owner completion` at the unchanged 60-second readiness budget. An isolated rerun also failed. Temporarily restored the original `start_with` installation order and original GET/retry verification implementation, reran the same protected-provider parent, and reproduced the same bound failure. Restored current implementation immediately. This isolates the failure from this ticket's startup/trust ordering; it is not a full original-revision or successful provider-lifecycle qualification. No test deadline was increased, provider fixture altered or unrelated worker code changed. The final focused native command explicitly filters that failing parent; its child remains intentionally ignored except when launched by its parent.
- Broad frontend parallel runs initially produced six, then two timing-sensitive failures. Isolated owner tests passed on both original and changed observation code; final single-worker/no-file-parallelism suite passed all 181 without unrelated source changes.
- Initial Svelte check lacked generated Paraglide output; app build generated it, then check passed.
- Final `cargo hack test --feature-powerset --all-targets` timed out after 200 seconds compiling dependencies (454/600). No full feature matrix pass is claimed.
- Final `just crap` timed out after 200 seconds compiling coverage dependencies (516/982, killed). No score or full coverage result is claimed.
- Cargo refreshed ticket-worktree lock entries for concurrently changed sibling `ofdb` path dependencies. Those incidental lock edits were restored, not included in this ticket. A final read-only main-worktree status also showed the same lockfile refresh (mtime 07:50). Initial main-worktree status was not captured, so its provenance is not asserted and it was left untouched to protect concurrent work. No main-worktree source edits, branch switch or commit was performed.
- No physically drawn desktop window/OS elevation prompt, real trust-store/provider/profile, WAN/container, macOS/Windows or production-platform acceptance. The rendered adapter test qualifies the progress/consent experience, not native GUI drawing timing. Source construction orders window creation before spawning host work.
- If elevation prevents killing an installer, close/reset retains ownership and waits for it to finish/reap; instantaneous shutdown is not guaranteed. Trust changes already made are not automatically rolled back or removed by reset.

## Integration

`git merge setup-simplification` reported `Already up to date` at `23e9006ca` after implementation and final focused checks. Integration ancestry is retained; only the ticket branch is committed.
