# Server configuration samples

Run commands from the repository root. Relative paths in these samples resolve against the process working directory, not the YAML file. Keep each advertised public base, listener, UI origin, and TLS identity consistent.

The samples describe current server entry points. They do not mean the accepted target architecture is fully implemented. See [target architecture](../docs/architecture.md).

## Independent server samples

| Command           | Default configuration           | Portless public base                | Direct-run listener | Data directory    |
| ----------------- | ------------------------------- | ----------------------------------- | ------------------- | ----------------- |
| `just idp`        | `config/idp/config.yaml`        | `https://idp.localhost:1355`        | `127.0.0.1:3000`    | `data/idp`        |
| `just management` | `config/management/config.yaml` | `https://management.localhost:1355` | `127.0.0.1:3001`    | `data/management` |
| `just device`     | `config/device/config.yaml`     | `https://device.localhost:1355`     | `127.0.0.1:3003`    | `data/device`     |
| `just storage`    | `config/storage/config.yaml`    | `https://storage.localhost:1355`    | `127.0.0.1:3002`    | `data/storage`    |

These recipes run each server through `portless`. Portless terminates HTTPS, assigns a dynamic loopback port through `PORT`, and publishes the public base through `PORTLESS_URL`. The server overrides `server.port` and `api_public_uri` from those environment variables, so the YAML values are fallbacks for direct `cargo run` without portless. Supply the portless public base URLs to browser or native clients. Independent setup and service operations are defined by the files under `openapi/`.

The current server entry points skip the independent setup ceremony when `PORTLESS_URL` or `PUBLIC_BASE_URL` is set. Run the IdP, Device, and Storage setup flows directly against the configured listener and advertised base, then use the portless recipes for ordinary serving. Current independent Management serves policy APIs and has no separate setup-owner ceremony. These current flows do not make Management the owner of Device records in the target model.

Preserve each server's retained data and TLS/setup state across restart. Do not place passwords, private keys, setup capabilities, or internal record IDs in these samples.

## Local composition

Local mode does not consume these YAML files:

```sh
just unified
```

It mounts `/idp`, `/management`, `/device`, and `/storage` at `https://unified.localhost:1355`. Portless terminates HTTPS and assigns a dynamic internal loopback port. Direct `cargo run -p unified-server -- local` serves plain HTTP on a fixed port. Local signer state is bound to the public issuer URL; use a fresh data directory when that URL changes. This mode is for development and does not establish production Installation membership.

Current local composition uses a shared embedded database implementation. That is a deployment choice, not an architectural requirement. One server process does not require one physical database file or one shared database host.

## Alternate configuration

Pass another configuration path before server arguments:

```sh
just device config/device/config.yaml serve --port 3103
```

An explicit listener override must agree with the advertised base and retained TLS identity. Environment overrides do not replace recovery of required retained state.
