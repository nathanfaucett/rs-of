# Password-manager reference application

This Svelte application demonstrates the platform's north-star workflow. It is intentionally small and is **not a qualified production password manager**.

## Current local behavior

- Vaults use the browser `@ofsdk/sql` package and IndexedDB.
- Core local vault operations do not require an IdP account or Storage service.
- The vault selector and local-only Sync Status panel link to the OIDC sign-in route.
- Vault records are encrypted with AES-256-GCM. The current key derivation uses PBKDF2-SHA-512 with 600,000 iterations.
- Locking clears the decrypted application view. Browser local storage is not an encrypted backup.
- The application includes local backup/recovery and offline-shell support; platform, eviction, update, and recovery limits still apply.

Use test secrets only. Do not infer production cryptographic, browser, or remote synchronization qualification from local CRUD tests.

## Target platform workflow

1. Create and use an encrypted vault offline.
2. Sign in to a Sync Server with OIDC.
3. Approve the Device and select the vault Resource for synchronization.
4. Synchronize ciphertext, not the master password or plaintext vault records.
5. Select the vault on a second Device, edit offline on both, and converge after reconnection.

This is the accepted target, not a claim that every step works now. See [product goal](../../docs/goal.md), [shared Device architecture](../../docs/architecture.md), and [current status](../../README.md#current-status).

OIDC authenticates the User and Application. It does not decrypt vaults, approve Devices, or select Resources.

## Local development

Use the toolchain versions in `rust-toolchain.toml`, `.nvmrc`, and the root contributor guide. From the repository root:

```bash
pnpm install --frozen-lockfile --ignore-scripts
pnpm e2e:prepare
pnpm --filter @ofsdk/password-manager dev
```

`pnpm e2e:prepare` builds local SQL/filesystem WASM and generated workspace dependencies. No sibling `ofdb` checkout is used. See [E2E setup](../../e2e/README.md) for prerequisites and validation limits.

Local vault use needs no service configuration. OIDC sign-in requires a matching IdP, OAuth Client, Application URI, redirect URI, and Storage URL. The isolated real-backend runner supplies and documents its [test configuration](../../e2e/README.md#browser-configuration-and-desktop-isolation); those test URLs are not production defaults.

The browser must allow IndexedDB, Web Crypto, Web Locks, and the relevant service-worker/cache APIs for offline delivery. Use HTTPS or a secure loopback development context.

## Reference-app scope

The platform milestones need basic encrypted CRUD, lock/unlock, backup/recovery, OIDC sign-in, Resource synchronization, and offline restart. Additional item types, import formats, generators, and production password-manager features are not platform completion criteria.

Do not add a second synchronization or identity model for this application. Use the public `ofdb` browser facade, OIDC client, Storage client, and shared Device/Selection contracts.

## Checks

```bash
pnpm --filter @ofsdk/password-manager test
pnpm --filter @ofsdk/password-manager check
pnpm --filter @ofsdk/password-manager test:backend
pnpm --filter @ofsdk/password-manager test:e2e
```

Backend tests start an isolated real local backend. They must not replace remote synchronization with local-only assertions. Record failures and unqualified flows in the relevant E2E evidence rather than claiming success from unit tests.
