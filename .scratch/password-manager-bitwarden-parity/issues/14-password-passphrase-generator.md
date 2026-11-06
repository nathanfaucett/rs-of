# 14: Generate passwords and passphrases

**What to build:** A local configurable generator with copy and explicit insertion into a Login draft without saving the item automatically.

**Blocked by:** 06 — Add item details and safe credential actions.

**Status:** resolved

**Type:** task

**Work status:** resolved

Claim: implement/password-manager-14, base b6ec0fdce. Scope: generator logic/component, minimal Login draft hookup, deterministic and visible-flow checks. Complexity: moderate. Risk: biased sampling, secret lifetime, unintended persistence. No storage/schema/workflow refactor.

- [x] Generate random passwords and passphrases using cryptographically secure randomness. Prefer existing suitable libraries/platform functions; document supported settings and any word-list source/license.
- [x] Provide accessible configuration, generation, copy, and explicit use-in-draft actions. Handle impossible settings and clipboard failures without exposing generated secrets in errors.
- [x] Insertion changes only the current draft and works with password confirmation/history rules. Cancel does not persist the generated value or modify another item.
- [x] Do not add persistent plaintext generated history or claim measured entropy without a justified model. Clear generated values on navigation/disposal/lock.
- [x] Test configuration constraints, invalid inputs, draft insertion, copy failure, cancel/save/reopen, and lock cleanup. Use focused deterministic logic checks, not flaky statistical assertions as security proof.
- [x] Ensure passphrase assets and all generator behavior work offline. Preserve existing encrypted backup/restore behavior.

## Answer

Completed in `implement/password-manager-14`. Recovered the unfinished task changes without resetting the worktree. Both the task HEAD and integration tip were `b6ec0fdce` at recovery; the integration base is an ancestor of the task.

- `src/lib/generator/generator.ts`: Web Crypto only, rejection sampling before modulo reduction, and whole-password rejection to satisfy selected character classes without forced-position bias. Password lengths 8–128; passphrases 4–20 independent word selections, four separators, optional capitalization and trailing random digit. Invalid settings fail before requesting randomness.
- `src/lib/generator/PasswordGenerator.svelte`: labeled settings, initial keyboard focus, masked output through existing `ProtectedValue`, explicit reveal/copy/clear/use actions, and fixed errors without exception text or secrets. No new dependency, runtime remote request, entropy claim, storage format, or persistent generated history.
- The complete 2,048-word BIP-39 English list and MIT license match pinned upstream revision `b57a5ad77a981e743f4167ab2f7927a55c1e82a8` byte-for-byte. Word-list SHA-256: `2f5eed53a4727b4bf8880d8f3f199efc90e58503646d9ff8eff3a2ed3b24dbda`. Source, settings, license, and limits are documented in `apps/password-manager/README.md`; the bundled license is also visible offline.
- `LocalSecretEditor.svelte`: explicit insertion updates only the active Login draft password and confirmation, masks it, and returns focus to Password. Existing save acknowledgement and dated history remain unchanged. Cancel leaves stored data intact; tests retain the other item and its nested data.
- Settings changes, clear, internal navigation, route navigation, page hide, disposal, and lock release generated state. Keyed protected output resets reveal and clipboard feedback. Late copy completion cannot restore discarded feedback. Clipboard contents can remain; no unsafe automatic clearing or guaranteed JavaScript erasure is claimed.

### Evidence

All paths below are under `apps/password-manager` unless specified.

- Recovery baseline: generator, component, editor, and mounted page checks passed (4 files / 22 tests). These were unfinished changes from before the crash, not a new red run.
- New TDD slice: `src/lib/generator/component.test.ts` failed because opening the generator left focus on the document body. A standard Svelte attachment now focuses Generate type; the test then passed.
- Final full Chromium suite: `node node_modules/vitest/vitest.mjs run --maxWorkers=1 --no-file-parallelism` passed, 16 files / 104 tests. Coverage includes deterministic sampling/constraints, word-list checksum, invalid settings, RNG/clipboard failures, masked output, explicit insertion, confirmation, cancel/save/reopen, dated history, other-item preservation, navigation/retirement, lock, and exact encrypted backup restore after removing source test storage.
- `svelte-kit sync` and `svelte-check --tsconfig ./tsconfig.json`: zero errors and warnings. Svelte autofixer: no issues or suggestions for both changed components.
- `biome lint .`: passed; existing generated Paraglide warnings/info remain. Existing mixed Vitest 4.1.10/browser 4.1.12 warning remains.
- `vite build && node scripts/check-offline.mjs`: passed. The server is stopped before password/passphrase generation. Checks cover bundled words/license, explicit insertion, confirmation, save/cancel, masked cold-process reopen, no generated value in application caches, zero server requests, and existing clean-profile encrypted backup/restore and storage-failure gates. Existing SQL/WASM dynamic-import and plugin-timing build warnings remain.
- Word list and license were independently fetched from the pinned upstream revision and compared byte-for-byte. Deterministic tests check all 2,048 unique words and the checksum; no statistical security claim is made.
- Every browser test/build/check used `flock /home/nathan/code/rust/aicacia/of/.scratch/password-manager-validation.lock`. One check batch timed out waiting for the shared lock; its replacement completed under the same lock. No timeout was relaxed inside a test.
- `git diff --check`: passed. No spec/map edit. No full Rust/monorepo build or `just crap`: that recipe requires a workspace SDK build and Rust coverage, outside the requested validation scope. No live vault or browser session used. Existing package symlinks, attachments, and screenshots remain untracked and are excluded from the commit.

## Comments

- 2026-11-05: Reclaimed the existing `implement/password-manager-14` worktree after the crash. Preserved all unfinished task files and untracked local package/artifact files. The spec already approves visible-flow, public-persistence, offline-browser, and focused generator test boundaries. Complexity: moderate; primary risks are sampling bias, retained plaintext, and unintended writes. Kept storage/schema/workflow ownership unchanged.
