# Server configuration samples

Run commands from the repository root. Relative paths in these samples resolve against the process working directory, not the YAML file. Keep each advertised public base, listener, UI origin, and TLS identity consistent.

The samples describe current server entry points. They do not mean the accepted target architecture is fully implemented. See [target architecture](../docs/architecture.md) and [backend deployments](../docs/backend-deployments.md).

## Independent server samples

| Command | Default configuration | Advertised HTTPS base | Data directory |
| --- | --- | --- | --- |
| `just idp` | `config/idp/config.yaml` | `https://127.0.0.1:3000` | `data/idp` |
| `just management` | `config/management/config.yaml` | `https://management-api.localhost:1355` | `data/management` |
| `just device` | `config/device/config.yaml` | `https://127.0.0.1:3003` | `data/device` |
| `just storage` | `config/storage/config.yaml` | `https://127.0.0.1:3002` | `data/storage` |

These recipes use `portless`. Supply the matching public base URLs to browser or native clients. Independent setup and service operations are defined by the files under `openapi/`.

Current independent Management serves policy APIs and has no separate setup-owner ceremony. Its sample public HTTPS base requires the configured proxy. IdP, Device, and Storage retain their current independent setup paths. These current flows do not make Management the owner of Device records in the target model.

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
