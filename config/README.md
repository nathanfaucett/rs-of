# Server configuration samples

Run from the repository root. Relative paths resolve against the working directory,
not the YAML directory. All samples select `https://of.localhost:1355` as the exact
app UI origin. Change it consistently for your actual browser/native deployment.

| Command | Configuration | HTTPS base | Data |
| --- | --- | --- | --- |
| `just unified` | `config/unified/config.yaml` | `https://127.0.0.1:3000` | `data/unified` |
| `just idp` | `config/idp/config.yaml` | `https://127.0.0.1:3000` | `data/idp` |
| `just management` | `config/management/config.yaml` | `https://management-api.localhost:1355` | `data/management` |
| `just device` | `config/device/config.yaml` | `https://127.0.0.1:3003` | `data/device` |
| `just storage` | `config/storage/config.yaml` | `https://127.0.0.1:3002` | `data/storage` |

Run Unified or standalone IdP, not both on port 3000. Unified explicitly mounts
`/idp`, `/management`, `/device`, `/storage` once. Its local HTTPS listener owns one
CA; it is not a Portless HTTP backend. Standalone setup owners keep their local
HTTPS listeners/authorities and use their configured root bases. Browser callers
need verified trust; CLI uses the actual CA path without installing OS trust.
Management is policy-only: its separate CLI serves loopback HTTP on port 3001, not
a setup listener/authority. Arrange a verified HTTPS proxy for its advertised
public base. This is not an additional setup owner or a fourth setup CA.

Configure the app's exact `PUBLIC_IDP_BASE_URL`, `PUBLIC_MANAGEMENT_BASE_URL`,
`PUBLIC_DEVICE_BASE_URL` and `PUBLIC_STORAGE_BASE_URL`. Bundled desktop derives
these from its one retained dynamic local HTTPS origin instead. Selecting remote
bases never substitutes local services or forwards a bundled setup code.

IdP, Device and Storage each expose `<base>/setup/owner/...` and issue their own
protected code. Management has no fourth setup code or registration ceremony.
Submit one public exact Create/Join tuple separately under each owner's code; only
IdP receives the Create password. Correlate each current status and public operation
before login. See [CLI examples](../docs/operations.md#automatic-unified-cli-bootstrap).

Preserve separate domain roots and each `<owner>.setup-host` directory. The sample
Unified TLS root is `data/unified/endpoint.tls`; the `endpoint_data_dir` selector
locates that authority, not an old endpoint credential file. The composed Device
worker owns installation endpoint material. Dynamic desktop also retains
`listen.addr`. There is no host metadata SQL database or bootstrap journal.
Existing/partial unsupported state is refused intact; do not delete evidence to
manufacture freshness. Owner-local stopped reset preserves sibling/shared SQL state;
hosted reset preserves the shared CA. See [reset/recovery](../docs/operations.md#persistence-reset-and-recovery).

Pass an alternate config as the first recipe argument, then server arguments:

```sh
just unified config/custom.yaml serve
just device config/device/config.yaml serve --port 3103
```

An explicit listener override must agree with the actual advertised HTTPS base and
retained TLS identity. Keep retained paths/URLs/origins stable. Environment overrides
remain supported but are not a substitute for recovering exact retained owner state.
No passwords, private keys, internal IDs or setup codes belong in these samples.
