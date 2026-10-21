# Default server configurations

Run from the repository root (`of/`). Config and data paths are relative to the
process working directory, not the YAML directory.

| Command           | Default config                  | Public API                              | Listener         | Data root           |
| ----------------- | ------------------------------- | --------------------------------------- | ---------------- | ------------------- |
| `just unified`    | `config/unified/config.yaml`    | `https://unified.localhost:1355`        | `127.0.0.1:3000` | `./data/unified/`   |
| `just idp`        | `config/idp/config.yaml`        | `https://idp-api.localhost:1355`        | `127.0.0.1:3000` | `./data/idp`        |
| `just management` | `config/management/config.yaml` | `https://management-api.localhost:1355` | `127.0.0.1:3001` | `./data/management` |
| `just storage`    | `config/storage/config.yaml`    | `https://storage-api.localhost:1355`    | `127.0.0.1:3002` | `./data/storage`    |

All configs allow the app origin `https://of.localhost:1355` (`pnpm apps:dev`).
Unified mounts `/idp`, `/management`, and `/storage`; standalone APIs have no
prefix. Unified and standalone IdP share port 3000: run one mode at a time.

Pass another config as the first argument, followed by any server arguments:

```sh
just unified config/custom.yaml serve
just idp config/idp/config.yaml serve --port 3100
```

`just unified` retains Portless routing and TLS termination. Portless's `PORT`
and `PORTLESS_URL` control the backend listener and public API base; the app
origin comes from the config, with no launcher lookup or injection.
Standalone recipes start native HTTPS listeners, not Portless routes. Arrange
HTTPS-capable forwarding for their listed public names; an ordinary HTTP
Portless backend cannot forward these HTTPS listeners. Native setup may require
local CA trust with explicit OS consent.

## Persistent state

Unified uses separate `endpoint`, `idp`, `management`, and `storage` roots under
`./data/unified/`. Bootstrap also derives adjacent `endpoint.host`,
`endpoint.credentials`, `endpoint.provisioning`, and `endpoint.tls` paths.
Standalone setup derives sibling `<owner>.setup-host` directories under `./data/`.
Database files, filesystem content, keys, and other generated state are managed
beneath their configured roots; they do not belong in `config/`.
`/data/` is ignored by Git. Never share live roots between deployment modes.

On fresh startup, leave domain roots absent; setup creates its own state.
Credentials, signer bindings, and Installation issuers are not prefilled. The
blank IdP issuer lets app-driven setup select and retain the real issuer.
Start the servers without a subcommand, retrieve their protected setup codes,
and choose Create or Join in the app. After starting neutral unified setup,
continue through the app rather than switching to CLI `create`/`join`.

Retained setup requires the same paths and host settings, including the effective
listener, URLs, origin, and audiences. Keep the Portless backend port stable.
Existing or partial state is never silently reset or migrated. Changing these
configs does not relocate existing databases; select the old config to use old
state. See [operations](../docs/operations.md) for continuation and recovery.

Existing config-loader environment overrides remain supported, but are not
required by these defaults. Remove stale overrides before fresh setup.

Validate the defaults with:

```sh
cargo test -p unified-server --lib config::tests::default_
```
