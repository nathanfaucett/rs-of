# TypeScript packages

Workspace packages for browser and application clients. Some packages wrap this
repository's Rust crates through WebAssembly.

## Package status

**Publishable** means that `package.json` does not mark the package as private. It
does not mean that the package is published to npm.

### Publishable packages

| Package                        | Directory                        | Purpose                                |
| ------------------------------ | -------------------------------- | -------------------------------------- |
| `@ofsdk/device-client`         | `packages/device-client`         | Device authority API client            |
| `@ofsdk/idp-client`            | `packages/idp-client`            | Identity provider API client           |
| `@ofsdk/management-client`     | `packages/management-client`     | Management API client                  |
| `@ofsdk/native-fetch`          | `packages/native-fetch`          | Native application fetch bridge        |
| `@ofsdk/oidc-client`           | `packages/oidc-client`           | OpenID Connect client                  |
| `@ofsdk/sql`                   | `packages/sql`                   | SQL over IndexedDB with WASM           |
| `@ofsdk/storage-client`        | `packages/storage-client`        | Storage REST and WebSocket client      |
| `@ofsdk/storage-db-collection` | `packages/storage-db-collection` | Storage-backed TanStack DB collections |
| `@ofsdk/svelte-headless`       | `packages/svelte-headless`       | Svelte 5 headless utilities            |

### Private workspace packages

These packages are for use through this pnpm workspace. They are not published to
npm and must not use registry installation or publication instructions.

| Package                      | Directory                      | Purpose                                     |
| ---------------------------- | ------------------------------ | ------------------------------------------- |
| `@ofsdk/file-system`         | `packages/file-system`         | Browser file system over OPFS and IndexedDB |
| `@ofsdk/kv`                  | `packages/kv`                  | Key-value storage over IndexedDB            |
| `@ofsdk/net`                 | `packages/net`                 | Browser mesh networking                     |
| `@ofsdk/sdk`                 | `packages/sdk`                 | Authenticated browser Storage access        |
| `@ofsdk/storage-sync-client` | `packages/storage-sync-client` | Storage SQL sync WebSocket transport        |
| `@ofsdk/sync-sdk`            | `packages/sync-sdk`            | Sync orchestration                          |

## Local development

Run these commands from the repository root. They require only this checkout.

```sh
pnpm install --frozen-lockfile
```

For `sql`, `kv`, `net`, and `file-system`, generated `pkg/`, `esm/`, and `types/`
output is not tracked. On a fresh checkout, create the WASM and TypeScript
output before a full package build or before consuming their generated entry
points:

```sh
pnpm --filter @ofsdk/sql run build:all
pnpm --filter @ofsdk/kv run build:all
pnpm --filter @ofsdk/net run build
pnpm --filter @ofsdk/file-system run build
```

Then build and test the direct package workspaces:

```sh
# Build every direct package workspace.
pnpm --filter "./packages/*" run build

# Install the browser used by the SQL tests on the first test run.
pnpm --filter @ofsdk/sql exec playwright install chromium

# Run tests for packages that define a test script.
pnpm --filter "./packages/*" run test

# Build or test one package.
pnpm --filter @ofsdk/sdk run build
pnpm --filter @ofsdk/sdk test
```

For later TypeScript-only changes, use each package's `build` script after its
WASM output exists. Other packages also generate entry points such as `esm/`,
`types/`, or `dist/`; build their workspace dependencies before running consumers.

## Related documentation

- [Rust database crates](../db/README.md)
- [Rust file system crate](../fs/README.md)
- [Rust mesh networking crate](../net/README.md)
- [Project goals](../docs/goal.md)
- [End-to-end tests](../e2e/README.md)
