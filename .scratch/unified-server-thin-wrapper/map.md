# Unified Server: Thin Wrapper Refactor

This refactor aims to strip `unified-server` of all "Independent" (local-only) logic, making it a pure composition layer.

## Domain Context

- **Independent Path**: The logic used by the CLI to perform node bootstrapping, provisioning, and joining. This currently lives in `unified-server` but belongs in `idp-server`.
- **Hosted Path**: The logic used when a node joins an existing, running `unified-server` via its HTTP API.

## Module Mapping

| Current Module (`unified-server`) | Target Module (`idp-server`)               | Purpose                     |
| :-------------------------------- | :----------------------------------------- | :-------------------------- |
| `local_join.rs`                   | `idp-server/src/local_join.rs`             | Node joining state machine  |
| `local_provisioning.rs`           | `idp-server/src/local_provisioning.rs`     | Node provisioning logic     |
| `bootstrap_config.rs`             | `idp-server/src/bootstrap_config.rs`       | Bootstrapping configuration |
| `bootstrap_verification.rs`       | `idp-server/src/bootstrap_verification.rs` | Bootstrapping verification  |
| `credential_bootstrap.rs`         | `idp-server/src/credential_bootstrap.rs`   | Credential generation       |
| `issuer_discovery.rs`             | `idp-server/src/issuer_discovery.rs`       | Identity issuer discovery   |
| `local_tls.s`                     | `idp-server/src/local_tls.rs`              | Local TLS handling          |

## Success Criteria

- `unified-server` contains no "local" or "independent" logic modules.
- `unified-server` CLI remains fully functional by consuming the relocated modules from `idp-server`.
- `unified-server` is a pure composition of `idp-server`, `management-server`, and `storage-server`.
