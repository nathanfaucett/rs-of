# Password manager parity integration map

## Notes

- 2026-11-05: Integration root `of`, branch `implement/password-manager-parity`, baseline `42391ef3c943365a2319b8118215e333d28d3e44`. No implementer worktree edits, parent spec changes, dependency installation, or push.
- Verified and merged in order: `implement/password-manager-01` (`0e59e8b519a3acdc271674805e11e3d031d2c99b`, fast-forward), `implement/password-manager-02` (`3c8831c2ccf1ca04605197e410af80fd47f03e29`, merge `d7e66f228`), `implement/password-manager-03` (`a25aa10dc18f8ff1722b56151857b6b2a5b754df`, merge `9cce7267b`), then `implement/password-manager-19` (`e45e8da4048c758e41550aad04c78a295c4e00ef`, merge `23ac8b403`). All four tips are ancestors of the integrated head.
- Complexity: medium. Risk: high at authentication/identity overlap because a wrong resolution could deny vault access or return an unauthenticated handle. Scope: combine existing mechanisms and update their existing public API, mounted-page, and built-app test boundaries; no new feature or compatibility path.
- Integrity checks compare migration 1, `Cargo.lock`, and the parent spec with baseline Git objects. All are unchanged. Authentication appends migration 2; no baseline SQL rewrite.

## Decisions-so-far

- [01: Reference baseline — resolved](issues/01-reference-baseline.md). Use [reference evidence](../../docs/password-manager-reference.md), [personal-json-v1](../../docs/password-manager-json.md), and [synthetic fixtures](../../docs/fixtures/password-manager/README.md). These define the supported subset, not observed full deployment parity. Ticket 01's triage remains `ready-for-agent`; its work status and Answer are resolved.
- [02: Vault authentication — resolved](issues/02-vault-authentication.md). Keep the versioned AES-GCM proof bound to the persisted ID, salt, IV, and creation time. Verify before returning a handle. Legacy populated vaults authenticate every record before proof upgrade; legacy empty vaults remain locked without setting any attempted password. Keep storage-write error handling and proof verification.
- [03: Vault identity — resolved](issues/03-vault-identity.md). Keep random stable IDs, independent database names, reservation before writes, metadata-ID checks, and registration completion only after metadata/proof persistence. Unlock takes an ID, never a display name. Preserve in-place legacy registration and interrupted-registration recovery.
- Auth/identity conflict resolution keeps both derivation validation and identity validation. Creation inserts metadata plus proof before completing registration; unlock verifies the proof before completing registration. Test schema expectations now use authentication version 2 and reject future version 3.
- Authentication tests resolve storage through registered IDs. Legacy fixtures return/register their original IDs. The creation-write fault uses a prepared database with a reserved synthetic ID so the real IndexedDB failure reaches metadata persistence, not database initialization. It also proves that retry creates a separate identity and leaves the interrupted reservation locked.
- [19: Offline startup — resolved](issues/19-offline-startup.md). Keep the production asset/WASM service worker, complete-version cache updates, safe storage guidance, and built-app browser harness. The harness now captures the selector's stable ID and reuses it after reload, update, and cold browser restart. Keep the package-resolved SQL test allowlist and local Vitest cache.

### Integration evidence

Commands run from the repository root unless stated otherwise:

- Initial `pnpm --filter @ofsdk/password-manager test`: 11 failures / 37 passes plus one unhandled injected fault. Causes: old name-based authentication calls, selector labels that now include IDs, and a fault injected before the intended metadata write. After fixes: **9 files / 48 tests pass** in Chromium using real SQL/WASM, IndexedDB, and Web Crypto; no unhandled error.
- `pnpm --filter @ofsdk/password-manager check`: passed; **0 errors, 0 warnings**. Also rerun as `pnpm run check` from `apps/password-manager` after formatting.
- `pnpm --filter @ofsdk/password-manager build`: passed, static application and database WASM emitted. SQL static/dynamic import and plugin timing notices remain non-blocking.
- `node scripts/check-offline.mjs` from `apps/password-manager`: first failed at the obsolete exact name-only selector. After ID-based selection: **all 12 scenario checks pass**. Includes stopped-server offline create/edit/lock/select/unlock, reload/tab restart, cold browser process with persisted data and zero server requests, cache privacy/WASM, failed and successful updates, deployment mismatch, evicted assets, first-install failure, injected denied/quota/unsupported-browser faults, and uncached first-offline-visit limits.
- `pnpm exec biome check --write src/lib/vault/authentication-fixtures.ts src/lib/vault/authentication.test.ts src/routes/authentication.test.ts scripts/check-offline.mjs` from the app: passed, 4 files checked; 3 formatted.
- Svelte autofixer on integrated `src/routes/+page.svelte`, Svelte 5.56.4: no issues or suggestions.
- `git diff --check`: passed. Git ancestry and baseline migration/lockfile/spec integrity assertions: passed. Failed-test screenshots and attachments generated by this integration run were removed.
- `cargo hack test --feature-powerset --all-targets --locked`: **timed out after 120 seconds during compilation**; no full Rust result.
- `just crap`: **timed out after 120 seconds during Rust compilation** after its SDK build prerequisite; no CRAP score or coverage result. Do not infer success from compilation progress.

## Fog

- No unresolved merge conflicts or known app test/check/build/offline failures after integration.
- Rust feature combinations and CRAP remain unqualified because both bounded commands timed out. A longer run is required if workspace-wide validation is needed; no Rust source changed here.
- [01](issues/01-reference-baseline.md): enabled reference tabs were unavailable. Selected server version/configuration and unlocked reference workflows remain unverified; inherited client version is not a fresh observation.
- [19](issues/19-offline-startup.md): Chromium/root-path/current Login flows only. Other engines, mobile/private modes, real disk exhaustion/eviction, non-root deployment, and the final typed-item release need qualification in [20](issues/20-offline-release-qualification.md).
- [04](issues/04-encrypted-backup-restore.md) still owns complete encrypted backup/restore. Preserve the authentication proof and bound metadata, or deliberately re-establish proof under a verified key. Current encrypted export is not a complete backup.
- Typed items and remaining parity work are not implemented by this merge. No independent cryptographic audit, rollback prevention, secure-erasure guarantee, or production readiness claim.
- Git reports an existing failed automatic-GC log and excess unreachable loose objects. No prune or repository-maintenance operation was run. Existing untracked `.scratch/worktrees/` is left intact.
