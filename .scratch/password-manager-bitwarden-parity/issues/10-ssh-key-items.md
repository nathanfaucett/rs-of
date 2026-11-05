# 10: Store SSH Key items

**What to build:** Complete local storage and protected viewing of SSH public/private key material as its own item type.

**Blocked by:** 05 — Transition existing records to typed Login items; 06 — Add item details and safe credential actions.

**Status:** resolved

**Type:** task

**Work status:** resolved

- [x] Implement the verified SSH Key field mapping and validation across the typed model, persistence, list, detail, and editor. Record supported key representations and unsupported inputs explicitly.
- [x] Support create, view, edit, cancel, save, and reopen without a required Login password. Preserve multiline key material and applicable metadata/custom fields without unintended normalization.
- [x] Mask private key material by default. Reveal/copy must be explicit, keyboard accessible, and reset on navigation and lock. Do not log keys or include them in error messages.
- [x] Test only synthetic keys: supported representations, malformed input, multiline preservation, cancellation, isolated updates, write failures, and real-persistence reopen.
- [x] Extend encrypted backup/clean-storage restore coverage for SSH Key and mixed-type vaults. Preserve acknowledged writes and complete-snapshot validation.
- [x] Preserve offline operation/assets. Do not add SSH key generation, an SSH agent, passkey support, or native integration.

## Answer

Implemented on `implement/password-manager-10`, based on integration `30c7f7c8c`. No spec or map changes.

### Acceptance and evidence

- **Verified mapping and validation:** reused the existing strict `SshKeySchema` and documented `personal-json-v1` mapping, not a new model. Added `SshKeyDraft` and direct SSH branches to draft construction, workflow, list creation, editor, and details. Name/private key must be nonblank. Optional public key/fingerprint retain omitted/null/empty states. Malformed types and unknown fields fail the complete snapshot without writes or value-bearing diagnostics.
- **Representations:** opaque text storage accepts OpenSSH, PEM, encrypted PEM, PuTTY-style text, and arbitrary nonblank text. Synthetic tests retain Unicode, multiline/CRLF, and leading/trailing whitespace. These checks do not establish format validity, usable keys, matching pairs, or a correct fingerprint. No binary-file parsing/import, key generation, agent, passkey, or native integration was added. See `docs/password-manager-items.md`, SSH Key section.
- **CRUD and preservation:** mounted real application tests create, view, edit, cancel, save, lock, and reopen without Login controls/password confirmation. They compare exact selected-item fields and common metadata, custom fields, dates, folder, encrypted provenance, history, and unchanged neighboring Login, Secure Note, Card, and Identity items. Key edits do not append Login history.
- **Protection and cleanup:** details reuse `ProtectedValue`. `SshKeyFields.svelte` keeps private text out of the DOM until explicit reveal. Navigation/cancel/save/lock reset reveal; keyboard controls work in the built offline app. Tests cover explicit successful/denied copy with fixed feedback and lock during draft, copy, read, write, and persisted acknowledgement. Late results cannot restore keys or success messages. Existing clipboard retention and JavaScript memory limits remain; no guaranteed erasure is claimed.
- **Write safety:** the real workflow/collection retains acknowledged writes, strict complete-snapshot validation, atomic existing storage staging, and authoritative reconciliation. Encryption and post-write readback faults produce no false saved message; refresh/reopen returns the authoritative record. Storage/backup schemas and migration numbers are unchanged.
- **Backup:** the new real-persistence suite exports all five item kinds with folders and encrypted lock settings, discards source storage, rejects the wrong password, restores into clean storage, and compares the complete logical snapshot plus exact encrypted records. The built offline backup check now includes SSH, verifies exact metadata/proof/ciphertext after source-profile disposal, and opens the restored key with keyboard controls.
- **Offline:** the built app check covers SSH creation/edit/cancel/save/reveal with networking disabled; reload and a new browser process with the server stopped; exact persisted fields; clean-profile backup recovery; and all existing generator, TOTP, search, folder, Trash, cache-update, missing-asset, and storage-failure checks. No new runtime package or separately fetched asset.

### Validation

Every browser/test/build/check command used `flock /home/nathan/code/rust/aicacia/of/.scratch/password-manager-validation.lock`. All Vitest commands used `--maxWorkers=1 --no-file-parallelism`.

- Red: the first mounted SSH test failed at missing **New SSH key** before implementation.
- Focused mounted editor/draft/SSH suites: **33 passed** before the three representation cases were added.
- Final app suite: `node node_modules/vitest/vitest.mjs run --maxWorkers=1 --no-file-parallelism` — **261 tests, 26 files passed**, including all **16 SSH cases**.
- App check: `node node_modules/@sveltejs/kit/svelte-kit.js sync` then `node node_modules/svelte-check/bin/svelte-check --tsconfig ./tsconfig.json` — **0 errors, 0 warnings**.
- Svelte autofixer for `SshKeyFields.svelte`, `LocalSecretEditor.svelte`, and `SecretDetail.svelte` — no issues or suggestions.
- New SSH test/browser-helper files: Biome format and lint passed; all 16 SSH cases and the zero-error app check passed again after formatting.
- Production `node node_modules/vite/bin/vite.js build` — passed.
- `node scripts/check-offline.mjs` — passed, including SSH and encrypted clean-profile recovery.
- Initial lock waits timed out without running tests. Initial `pnpm run` replaced worktree dependency links and failed package resolution; restored local ignored links and used installed tools directly. No manifest/lockfile change. Built-browser test label/filter mistakes were corrected before its passing run.

No full Rust/monorepo validation, `just crap`, live credentials, key generation/use, other browser engines, independent crypto audit, or guaranteed clipboard/memory erasure. Scope and risk remain in the approved item/storage contracts.

### Merger notes

Keep direct kind branches and union members from parallel tickets 08 (Card) and 09 (Identity). Shared files are `secret-draft.ts`, `local-workflow.svelte.ts`, `LocalSecretEditor.svelte`, `SecretDetail.svelte`, item docs, and the offline/backup check entry points. SSH fields are isolated in `SshKeyFields.svelte`; synthetic browser actions are isolated in `check-ssh-key.mjs`. Preserve all existing generator/TOTP/search/folder/Trash/lock/backup/recovery branches and assets. No storage schema or migration conflict is introduced. Ran `git merge --no-edit implement/password-manager-parity` on the task branch: **Already up to date**, integration still `30c7f7c8c`. No root merge or push.

## Comments

- 2026-11-05: Claimed on `implement/password-manager-10`, based on integration `30c7f7c8c`. Using TDD, svelte-code-writer, svelte-core-bestpractices, and ponytail. Test seams are the spec-approved mounted application and public real-persistence vault API. All validation uses the shared flock; Vitest uses one worker and no file parallelism.
