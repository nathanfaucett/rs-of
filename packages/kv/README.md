# @ofsdk/kv

Private workspace TypeScript wrapper for browser key-value storage over
IndexedDB. The WASM module is built from `db/wasm`. This package is not published
to npm.

## Main exports

- `Database` opens a named database and creates clients.
- `Client` reads, writes, scans, and starts transactions.
- `Transaction` holds operations until `commit()` or `rollback()`.

The package also exports error, value, and synchronization types. See
[`src/index.ts`](src/index.ts) for the complete export list.

## Usage

```ts
import { Database } from "@ofsdk/kv";

const database = await Database.open("settings");
const client = database.client();

await client.set("theme", "dark");
const theme = await client.get("theme");

const transaction = await client.transaction();
await transaction.set("language", "en");
await transaction.commit();
```

Values must be JSON-serializable. `set` accepts an optional expiry timestamp in
Unix epoch milliseconds.

## Build and checks

Run from the repository root:

```sh
pnpm install --frozen-lockfile
pnpm --filter @ofsdk/kv run build:all
pnpm --filter @ofsdk/kv run check
pnpm --filter @ofsdk/kv run lint
```

`build:all` builds the WASM module and the TypeScript and browser outputs. The
generated `pkg/`, `esm/`, `types/`, and `browser/` directories are not tracked.
After the WASM output exists, use `pnpm --filter @ofsdk/kv run build` for a
TypeScript-only rebuild. This package has no `test` script.

## Related

- [Rust KV crate](../../db/kv/README.md)
- [Package index](../README.md)
