# 15: Generate TOTP codes locally

**What to build:** Local code generation and countdown for stored authenticator keys, with safe copy and explicit unsupported-input errors.

**Blocked by:** 06 — Add item details and safe credential actions.

**Status:** ready-for-agent

**Type:** task

**Work status:** resolved

- [x] Parse supported raw secrets and otpauth URIs. Validate algorithm, digit count, and period; explicitly reject malformed/unsupported inputs without disclosing the key in messages.
- [x] Show the current code and time remaining in the item detail flow, with accessible copy and failure feedback. Retain protected handling of the underlying secret.
- [x] Update at time boundaries and immediately reconcile on tab resume. Dispose timers and remove codes/keys from application references on navigation and lock.
- [x] Verify supported algorithm/digit/period combinations against known-answer vectors and cover malformed URIs, boundary times, suspended/resumed tabs, and clipboard failure.
- [x] Test save/reopen and backup/restore preservation of stored keys without rewriting key material merely because it was displayed.
- [x] Operate entirely locally/offline; do not send keys to a service or present this as a second cryptographic factor for local vault unlock. Reuse an existing appropriate dependency if available.

## Comments

- Claimed on `implement/password-manager-15`, base `b6ec0fdce`, in the assigned worktree. Scope: dedicated TOTP module/component and small SecretDetail hookup; preserve ticket 07/14/16 ownership. Approved seams: mounted application flows, public vault API with real persistence, and focused known-answer TOTP/parser tests. Complexity: moderate; risk: secret exposure and stale asynchronous code/copy results; priority: strict validation and lifecycle cleanup. No map/spec edits or live tabs.
- Recovered after the crash. Preserved and reviewed all existing source changes and this claim. Confirmed `b6ec0fdce` is an ancestor of the task and the integration branch. Loaded TDD and both Svelte skills. Used the approved seams above. All browser tests, build, check, and Svelte tooling ran in sequence under `flock /home/nathan/code/rust/aicacia/of/.scratch/password-manager-validation.lock`; Vitest used `--maxWorkers=1 --no-file-parallelism`.

## Answer

Implemented local TOTP with platform Web Crypto HMAC; no dependency or persistence-format change.

### Acceptance evidence

- `apps/password-manager/src/lib/secrets/totp.ts`: parses raw Base32 (case, grouping spaces, canonical padding) and `otpauth://totp/` URIs. Supports SHA1/SHA256/SHA512, 6/7/8 digits, and integer periods 1–86400 seconds. Rejects bad Base32, padding/trailing bits, invalid URI encoding, HOTP, duplicate/unknown parameters, and unsupported settings. All parser messages are fixed text, without input or keys.
- `src/lib/secrets/totp.test.ts`: all 18 RFC 6238 Appendix B vectors, RFC 4226 boundary values, and all three algorithms crossed with 6/7/8 digits and periods 1/30/60/86400. Covers invalid times and 35 malformed/unsupported inputs with non-disclosure assertions.
- `src/lib/secrets/TotpCode.svelte` and `SecretDetail.svelte`: read-only local code, named countdown, keyboard-accessible copy, success/error feedback, protected key unchanged, and explicit device-clock/local-unlock scope. Timer updates use wall time. Visibility, pageshow, and focus reconcile immediately. Copy checks expiration before writing. Attachment cleanup cancels timers, clears code/feedback and decoded bytes, releases parsed parameters, and invalidates late signing/copy results.
- `src/routes/totp.test.ts`: 12 mounted-application tests cover boundaries, suspended/resumed tabs, expired copy, clipboard denial with a secret-bearing error, cryptography failure, slow cryptography, navigation/lock during pending signing/copy, and exact key preservation. Raw lowercase/spaced keys, a custom URI, and an unsupported URI survive display, edit/save, reopen, encrypted backup, source-database deletion, and clean restore. Read-only display leaves encrypted records unchanged.
- TDD evidence: the added slow-cryptography test first failed with a 30-second countdown instead of 25 seconds. Recomputing the countdown at completion fixed the root cause. The focused suite then passed all 58 tests.
- `scripts/check-offline.mjs`: built app reloads offline with the server stopped, saves a synthetic TOTP key, checks known codes/countdown across a boundary, and reopens the stored Login with the key still protected. Zero server requests. Existing cold-process startup and clean-profile encrypted backup/restore checks also pass. The first offline attempt exposed an advancing test-clock issue; fixed clocks make the known-answer assertions stable.
- `LocalSecretEditor.svelte`: only the obsolete TOTP help text changed; no generator, history, or lock-setting behavior changed.

### Validation

From `apps/password-manager`, with the lock above:

- `pnpm exec vitest run --maxWorkers=1 --no-file-parallelism`: **149 passed, 15 files**.
- `pnpm run check`: **0 errors, 0 warnings**.
- `pnpm run test:offline`: production build passed; the initial offline test-clock assertion failed. After fixing the harness, `node scripts/check-offline.mjs` passed every check against that build, including TOTP, zero-network operations, cold startup, cache-update failures, and clean-profile restore.
- Svelte MCP `svelte-autofixer`: no issues or suggestions for `TotpCode.svelte`, `SecretDetail.svelte`, or `LocalSecretEditor.svelte`.
- Biome formatting applied only to the new TypeScript tests/module and touched offline script. `git diff --check` passed.

Existing tooling warnings remain: Vitest 4.1.10/browser 4.1.12 mismatch and the SQL WASM static/dynamic import warning. No Rust, full-monorepo, or CRAP run: these are excluded by this task, and CRAP invokes workspace Rust coverage. No live vault, shared map/spec, screenshots, or browser artifacts are included. Clipboard content is not automatically cleared, as stated in the UI. JavaScript memory zeroization is not guaranteed.
