# TypeScript SDK Packages

TypeScript/JavaScript SDKs for the `of` ecosystem, built from the Rust crates via `wasm-pack`.

## Packages

| Package | Source | Description |
|---------|--------|-------------|
| `@ofsdk/sql` | `packages/sql` | SQL database client (from `db/wasm`) |
| `@ofsdk/sql-wasm` | `packages/sql-wasm` | Pre-built WASM bundle for `@ofsdk/sql` |
| `@ofsdk/kv` | `packages/kv` | Key-value store client (from `db/kv-wasm`) |
| `@ofsdk/net` | `packages/net` | Mesh networking client (from `net/iroh-chain-wasm`) |
| `@ofsdk/file-system` | `packages/file-system` | Filesystem client (from `fs/file-system-wasm`) |
| `@ofsdk/example` | `packages/example` | Example app integration |
| `@ofsdk/fs-wasm` | `packages/fs-wasm` | Pre-built WASM bundle for filesystem |
| `@ofsdk/sync-sdk` | `packages/sync-sdk` | Sync orchestration SDK |
| `@ofsdk/storage-client` | `packages/storage-client` | Storage API client |
| `@ofsdk/storage-db-collection` | `packages/storage-db-collection` | Database collection helpers |
| `@ofsdk/storage-sync-client` | `packages/storage-sync-client` | Storage sync client |
| `@ofsdk/oidc-client` | `packages/oidc-client` | OAuth/OIDC client |
| `@ofsdk/idp-client` | `packages/idp-client` | IdP API client |
| `@ofsdk/management-client` | `packages/management-client` | Management API client |
| `@ofsdk/device-client` | `packages/device-client` | Device API client |
| `@ofsdk/svelte-headless` | `packages/svelte-headless` | Svelte 5 headless components |
| `@ofsdk/native-fetch` | `packages/native-fetch` | Native fetch polyfill |
| `@ofsdk/sdk` | `packages/sdk` | Combined SDK re-export |

## Build

```bash
# Build all WASM packages
pnpm sdk:build

# Build individual package
cd packages/sql
pnpm build:wasm

# Build with specific rust toolchain (pinned in rust-toolchain.toml)
pnpm install --frozen-lockfile
pnpm sdk:build
```

## WASM Sources

| Package | Rust Source |
|---------|-------------|
| `@ofsdk/sql` | `db/wasm` → `packages/sql-wasm` (checked-in) → `packages/sql` wrapper |
| `@ofsdk/kv` | `db/kv-wasm` (if exists) → `packages/kv` wrapper |
| `@ofsdk/net` | `net/iroh-chain-wasm` → `packages/net` wrapper |
| `@ofsdk/file-system` | `fs/file-system-wasm` → `packages/file-system` wrapper |

The `wasm` crates (`db/wasm`, `net/iroh-chain-wasm`, `fs/file-system-wasm`) are standalone `wasm-pack` workspaces with their own `Cargo.lock`. They are **excluded** from the Cargo workspace.

## Development

```bash
# Install dependencies
pnpm install --frozen-lockfile

# Build all packages
pnpm sdk:build

# Run tests
pnpm test

# Type-check
pnpm typecheck

# Lint
pnpm lint
```

## Publishing

Packages are published to npm from the `packages/` directory. Pre-built WASM bundles (`sql-wasm`, `fs-wasm`) are checked in to avoid requiring Rust toolchain at install time.

```bash
# Publish single package
cd packages/sql
npm publish

# Or use changesets for versioning
pnpm changeset
pnpm version
pnpm publish -r
```

## Usage

```typescript
import { createSqlClient, createKvClient, createNetClient, createFileSystemClient } from '@ofsdk/sdk';

// SQL
const sql = await createSqlClient({ memory: true });
await sql.execute("CREATE TABLE users (id UUID PRIMARY KEY, name TEXT)");
await sql.execute("INSERT INTO users (id, name) VALUES (uuid_v7(), 'Alice')");
const rows = await sql.query("SELECT * FROM users");

// KV
const kv = await createKvClient({ memory: true });
await kv.set("key", { value: "hello", expiry: 1000 });
const value = await kv.get("key");

// Mesh networking
const net = await createNetClient();
await net.connect(trustedPeer);
net.onMessage((msg) => console.log("Received:", msg));

// Filesystem
const fs = await createFileSystemClient({ path: "/data" });
await fs.write("/hello.txt", new TextEncoder().encode("Hello"));
const content = await fs.read("/hello.txt");
```

## Related

- [db/README.md](../db/README.md) — Rust database crates (`ofdb-sql`, `ofdb-kv`)
- [fs/README.md](../fs/README.md) — Rust filesystem crate (`file-system`)
- [net/README.md](../net/README.md) — Rust mesh networking crate (`iroh-chain`)
