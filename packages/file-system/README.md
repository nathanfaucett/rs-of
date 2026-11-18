# @ofsdk/file-system

Private workspace TypeScript wrapper for the browser file system. It uses OPFS
for content and IndexedDB for metadata. The WASM module is built from
`fs/file-system-wasm`. This package is not published to npm.

## Main export

`FileSystem` opens a named file system and provides file, directory, scan,
revision, residency, and lifecycle operations. The package also exports the
`Entry`, `FileMeta`, `Residency`, and `ScanPage` types from
[`src/index.ts`](src/index.ts).

## Usage

```ts
import { FileSystem } from "@ofsdk/file-system";

const fileSystem = await FileSystem.open("example");
await fileSystem.createDir("/documents");
await fileSystem.write(
  "/documents/hello.txt",
  new TextEncoder().encode("Hello, world!"),
);

const content = await fileSystem.read("/documents/hello.txt");
console.log(new TextDecoder().decode(content));
fileSystem.close();
```

## Build and checks

Run from the repository root:

```sh
pnpm install --frozen-lockfile
pnpm --filter @ofsdk/file-system run build
pnpm --filter @ofsdk/file-system run check
pnpm --filter @ofsdk/file-system run lint
```

`build` builds the WASM module and TypeScript output. The generated `pkg/`,
`esm/`, and `types/` directories are not tracked. This package has no `test`
script.

## Related

- [Rust file system crate](../../fs/file-system/README.md)
- [Package index](../README.md)
