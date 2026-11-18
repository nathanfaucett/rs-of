# @ofsdk/net

Private workspace TypeScript wrapper for browser mesh networking. The WASM
module is built from `net/iroh-chain-wasm`. This package is not published to npm.

## Main exports

- `Endpoint` owns the local endpoint, peer allowlist, and pairing operations.
- `Mesh` sends and receives messages and opens streams. Obtain it from
  `endpoint.mesh`.
- `Stream` writes and reads bytes on a stream returned by `Mesh.openStream()`.

See [`src/index.ts`](src/index.ts) for the supporting types and errors.

## Usage

```ts
import { Endpoint } from "@ofsdk/net";

const endpoint = new Endpoint();
await endpoint.ready;

const unsubscribe = endpoint.mesh.onMessage((message) => {
  console.log(new TextDecoder().decode(message.payload));
});
await endpoint.mesh.broadcast(new TextEncoder().encode("hello"));

unsubscribe();
await endpoint.close();
```

Await `endpoint.ready` before reading `endpoint.id` or `endpoint.peers`.

## Build and checks

Run from the repository root:

```sh
pnpm install --frozen-lockfile
pnpm --filter @ofsdk/net run build
pnpm --filter @ofsdk/net run check
pnpm --filter @ofsdk/net run lint
```

`build` builds the WASM module and TypeScript output. The generated `pkg/`,
`esm/`, and `types/` directories are not tracked. This package has no `test`
script.

## Related

- [Rust mesh networking crate](../../net/iroh-chain/README.md)
- [Package index](../README.md)
