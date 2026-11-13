# Server configuration samples

Run from the repository root. Relative paths resolve against the working directory,
not the YAML directory. All samples select `https://of.localhost:1355` as the exact
app UI origin. Change it consistently for your actual browser/native deployment.

| Command | Configuration | HTTPS base | Data |
| --- | --- | --- | --- |
| `just idp` | `config/idp/config.yaml` | `https://127.0.0.1:3000` | `data/idp` |
| `just management` | `config/management/config.yaml` | `https://management-api.localhost:1355` | `data/management` |
| `just device` | `config/device/config.yaml` | `https://127.0.0.1:3003` | `data/device` |
| `just storage` | `config/storage/config.yaml` | `https://127.0.0.1:3002` | `data/storage` |

Unified local mode does not use YAML. Run `just unified --data-dir .data --port 1355
--origin http://localhost:5173 --no-tls`. It mounts `/idp`, `/management`, `/device`,
and `/storage` once. Omit `--no-tls` for a retained self-signed leaf certificate;
there is no CA or admission flow. See the [local backend guide](../README.md#local-development-backend).
The remaining configuration and setup instructions apply only to separate owners.

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

Preserve separate owner domain roots and each `<owner>.setup-host` directory.
See [reset/recovery](../docs/operations.md#persistence-reset-and-recovery).

Pass an alternate config as the first recipe argument, then server arguments:

```sh
just device config/device/config.yaml serve --port 3103
```

An explicit listener override must agree with the actual advertised HTTPS base and
retained TLS identity. Keep retained paths/URLs/origins stable. Environment overrides
remain supported but are not a substitute for recovering exact retained owner state.
No passwords, private keys, internal IDs or setup codes belong in these samples.
