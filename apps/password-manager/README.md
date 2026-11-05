# Password manager: local encrypted SQL vaults

The app uses the browser `@ofsdk/sql` database. Vault records are encrypted with
AES-256-GCM using a key derived from the master password with PBKDF2-SHA-512
(600,000 iterations). Core vault operations do not require an IdP account or a
Storage service. Use test secrets only; this is a development app, not a qualified
production password manager.

## Local setup

From the workspace root, install dependencies and build the linked SQL package:

```sh
pnpm install --frozen-lockfile
pnpm --dir ../ofdb --filter @ofsdk/sql run build:all
pnpm --filter @ofsdk/password-manager dev
```

The current dependency uses the sibling `ofdb` checkout. The browser must allow
IndexedDB, Web Crypto, and Web Locks. Local development must use a secure browser context.

## Local vault identities

Display names are labels, not storage keys. New vaults use a random stable ID and
an independent SQL database. Case, punctuation, whitespace, and duplicate names
cannot merge vaults. Names are trimmed; the selector shows the full ID to make
identical names distinct. `unlockVault(password, id)` accepts that ID, not a name.

The IndexedDB registry reserves each ID and database name with unique constraints
before creation writes vault data. It marks registration complete only after the
vault metadata is saved. Interrupted registrations stay visible. Unlock recovers
a saved vault only when its metadata ID matches the reservation. If metadata was
not saved, create a new vault with a new ID. Retrying creation never adopts the
interrupted vault. Storage errors are not success; reload the selector and check
browser permissions and free space. Do not clear browser data as a repair step.

Existing v1 named databases are registered in place with their stored vault ID.
No secret, salt, migration history, or source database is copied or deleted. The
old normalized name is the only available display label. Failed registration
leaves the source unchanged and can be retried. A duplicate stored ID in different
databases fails explicitly instead of selecting one silently. Unsupported old
development schemas still need separate recovery; this transition does not
repair them. There is no remaining name-based creation or unlock path.

## Typed items

Login supports optional credentials, ordered websites and match settings, notes,
favorites, custom fields, and dated password history. Only Login has an editor.
All five item types have validated encrypted storage and backup support. See
[typed fields, safe transition, and later-ticket interfaces](../../docs/password-manager-items.md).
The transition retains the original database and an authenticated pre-transition
backup. It never publishes an unverified or partial record set. Close old app tabs
before upgrade; retained source/staging data is not automatically removed.

## Encrypted backup and recovery

Unlock a vault and select **Download encrypted backup**. To recover, select
**Restore encrypted backup** in a clean browser profile. Validate the file with
its original master password, check the full vault ID and count, then confirm.
Existing vault IDs are never overwritten. Keep the file outside browser storage
and verify the recovered data before discarding it.

See [the backup format and recovery contract](../../docs/password-manager-backup.md)
for authenticated contents, API use, limits, atomic publication, uncertain outcomes,
and the real-persistence/clean-profile test evidence. Old record-only exports are
not complete backups. No raw key or plaintext item is exported.

## Offline delivery

Run `pnpm --filter @ofsdk/password-manager test:offline` to build and qualify the
static application. The test needs the installed Playwright Chromium browser.
It starts a temporary loopback static server, uses only synthetic vaults in an
isolated temporary browser profile, and closes browsers/server in `finally`.
It does not connect to the live comparison browser or use Playwriter.

In a worktree with symlinked dependencies, pnpm can reject its task-state path.
Run the same tools directly from `apps/password-manager`:

```sh
node node_modules/vite/bin/vite.js build
node scripts/check-offline.mjs
node node_modules/vitest/vitest.mjs run
node node_modules/@sveltejs/kit/svelte-kit.js sync
node node_modules/svelte-check/bin/svelte-check --tsconfig ./tsconfig.json
```

### Asset convention

`src/service-worker.ts` uses SvelteKit's production `build`/`files` manifest.
Every generated chunk, stylesheet, and database WASM asset is precached, even
when a feature has not yet been opened. New features must import their assets
through Vite or put public, non-sensitive assets in `static`. Do not fetch
required local feature assets from a CDN or add private data to `static`.
The worker caches only these public assets and the static root application
shell. It never caches API responses, decrypted items, keys, drafts, or exports.
It handles only same-origin GET requests for the allowlist; other requests are
not cached. No runtime response cache is used.

Serve the complete static `build` directory on HTTPS (loopback is sufficient for
tests), with correct JavaScript and `application/wasm` MIME types. Do not serve
an HTML fallback for missing JS/WASM files. Publish each deployment atomically;
keep old hashed assets available while old tabs remain open. Use a unique
SvelteKit deployment version for each build. Service workers are not installed
by the development server. A first-ever offline visit is not supported.

### Readiness, updates, and recovery

Wait for **Offline ready** before disconnecting. The layout checks local storage
and the active worker's complete asset set. It shows storage denial, quota,
unsupported-browser, failed-install, missing-asset, and waiting-update guidance.
The check does not promise future free space or prevent eviction. Browser storage
can be evicted; private browsing can discard it on close. Local storage is not a
backup. Keep an external encrypted backup and test clean-profile recovery.

An install checks the deployment version before and after fetching assets. It
caches one complete version or removes only its failed application cache. It does not activate early. Existing tabs keep the old complete version
until all controlled tabs close. Lock the vault, close all app tabs, and reopen
when an update is ready. Activation deletes only this application's old asset
caches at its base path, never IndexedDB vaults or other applications' caches.
A missing cached asset returns a clear 503 instead of mixing files from another
deployment. Reconnect for a complete new deployment. If the installed version's
cache was evicted and no new deployment exists, use browser developer tools to
unregister only this app's service worker and delete only its
`password-manager-app-…` Cache Storage entries, then reload online. **Do not
clear site data or IndexedDB**: that deletes source vaults. A fully evicted shell
cannot show an in-app warning while offline; reconnect to load it.

### Qualification and extension

`test:offline` checks the built root-path app in Chromium with real SQL/WASM,
IndexedDB, and Web Crypto. It checks offline create/edit/lock/select/unlock,
reload, tab restart, and a new browser process using the same test-only profile.
The static server is stopped during the first offline flow and the cold browser
start; those flows complete with zero server requests. It also checks:

- WASM precaching and absence of recognizable synthetic passwords/item values
  from the application cache;
- failed WASM installs/updates, deployment changes during install, retained old
  shell, waiting update, complete-version
  activation, unrelated-cache retention, and source vault preservation;
- evicted WASM and a first-ever uncached offline visit;
- actionable storage-denial/quota/browser-support states through injected browser
  API faults (not real disk exhaustion or browser policy testing).

Later feature tickets should extend this script's visible workflows using only
synthetic items. Retain the real storage, stopped-server, update, and cold-process
checks. This is the current Login baseline, **not** final typed-item release or
Bitwarden parity qualification. Other browser engines, private modes, mobile
browsers, eviction policies, and non-root deployments need separate qualification.

## Schema migrations

`src/lib/vault/migrations.ts` is the complete ordered SQL migration list.
`createVault()` and `unlockVault()` call `Client.migrate()` before accessing vault
metadata or secret records. Database open alone does not run migrations.

Version 1 creates:

- `vault_meta`: a fixed metadata key (`id = 1`), the vault ID, derivation salt,
  initialization vector, and creation timestamp. Duplicate creation fails instead
  of adding another metadata row or replacing the original salt.
- `secrets`: record ID, ciphertext, per-record nonce, and timestamps. Secret
  contents remain encrypted; no plaintext secret index is added.

Version 2 adds nullable `vault_meta.authentication`. The v1 migration is unchanged.
New vaults store metadata and a versioned AES-GCM password proof in one insert.
Unlock verifies that proof before returning a handle, including after the last
item is deleted.

### Authentication and existing vaults

The proof uses the existing non-exportable derived key and a fresh random
96-bit nonce. Its encrypted payload contains a purpose label, proof version,
vault ID, salt, original metadata IV, and creation time. Unlock checks both the
GCM tag and the exact payload. This binds the proof to the vault metadata; it
stores neither the password nor the key. The original metadata IV remains for
format continuity; it is not reused as the proof nonce.

A migrated v1 vault has a null proof. A populated v1 vault must decrypt **every**
record before unlock can write a proof. This is a one-way transition, not a
second writer: new vaults always have a proof. Wrong passwords or damaged records
do not establish one. The upgrade changes only the proof field, then reads and
verifies it before returning a handle. Failed writes leave unlock blocked and
report that the user must check local storage and reopen before retrying. A
rejected write can have an uncertain durable outcome; no rollback is assumed.

A legacy empty vault has no original-password evidence. It stays locked for
**every** attempted password. No attempt establishes a new password. Keep the
source unchanged and create a separate new vault. This rule does not affect new
empty vaults or upgraded vaults whose last item was removed. Malformed or unknown
proof formats fail closed; they are never treated as missing legacy proofs.

This mechanism authenticates key possession and metadata, not the completeness
or freshness of the record set. It does not prevent storage rollback, deletion,
a compromised browser, or offline password guessing. It is not an independent
security audit. Error messages contain no passwords, keys, or decrypted records.

The SQL runner owns `_ofdb_migrations`. Each migration and its history record
commit atomically. Reopening validates history and skips completed migrations.
Changed migration contents, unknown applied versions, and malformed history fail
without reset or repair. Writes use named columns and bound parameters so future
column additions do not depend on positional inserts or hand-built SQL literals.

### Adding a migration

Append the next contiguous version to `vaultMigrations`. Never edit an applied
migration, including its whitespace. Use only SQL supported by `ofdb`; current
ALTER support is limited to `ADD COLUMN`. Update queries and tests with the
schema change. Do not add another schema-version table or migration runner.

A failed commit can have an unknown durable outcome. Do not replay its SQL or
edit migration history. A later explicit create/unlock attempt checks history
again. Earlier migrations remain committed if a later migration fails.

### Development reset

Old development databases have no supported upgrade path. They are not adopted,
repaired, or deleted automatically. Clear their IndexedDB database explicitly in
browser developer tools, then create a new vault. This permanently deletes their
contents. The database name starts with `ofdb:sql:password-manager-vault-`.

## Collection behavior and limits

The local collection retains the official TanStack binding, validates decrypted
records, and reloads the complete vault after writes. Schema migrations do not
change that full-vault reload cost. Secret payload validation and SQL schema
migration are separate concerns; the SQL runner does not decrypt records.

Existing development limits remain:

- The current encrypted export is not a self-contained backup or restore format.
- Deletion is not secure erasure. SQL history, browser storage, and backups can
  retain prior ciphertext. No JavaScript memory-zeroization guarantee is made.

Do not log passwords, keys, decrypted records, or SQL parameters.

## Checks

```sh
pnpm --filter @ofsdk/password-manager test
pnpm --filter @ofsdk/password-manager build
pnpm --filter @ofsdk/password-manager check
```

Vault tests use real browser SQL, migrations, IndexedDB, and Web Crypto. They cover
migration history, repeated startup, old/newer schema rejection, colliding and
duplicate names, concurrent creation, interrupted registration, storage failure,
existing-vault registration, encrypted CRUD, and lock/unlock persistence.
Authentication tests cover empty and populated vaults, last-item removal, v1
upgrades, unprovable legacy empty vaults, damaged proofs, metadata binding, and
actual IndexedDB write failures. Mounted page tests use the real vault, workflow,
collection, and persistence to create, select by ID, lock, and reopen isolated
records, reject wrong passwords, and show legacy-empty guidance. Collection and
editor tests cover the existing local workflow. These checks do not qualify
production vault security, secure erasure, backup recovery, or large-vault
performance.
