# Backend deployment boundaries

## Local unified development

`unified-server local` composes IdP, Management, Device, and Storage under `/idp`, `/management`, `/device`, and `/storage`. The CLI requires a fixed loopback listener and seeds development identities.

```bash
just unified
```

The recipe exposes `https://unified.localhost:1355` through Portless. Portless terminates HTTPS and assigns a dynamic internal loopback port. Direct `cargo run -p unified-server -- local` serves plain HTTP on a fixed port. Production TLS belongs at nginx or a load balancer. Local signer state is bound to the public issuer URL; use a fresh data directory when that URL changes.

The current local implementation retains state under the selected data directory and may use one embedded database host. That is an implementation choice, not an architectural requirement for one physical file or public shared-host APIs.

Local mode does not establish Installation membership or qualify production Node setup. It uses seeded test identities. Private development keys are not encrypted at rest. Stop the server before copying its database for backup.

Do not expose the listener or seeded credentials to an untrusted network.

## Independent owners

IdP, Device, and Storage retain independent setup, Create/Join, reset, and TLS paths. Management consumes identity and policy and is not a separate setup authority. Their APIs remain separate even when a host composes them in one process.

Independent deployment, WAN behavior, OS trust integration, and two-Node offline restart are not qualified by local unified tests. Use `config/README.md` for sample configuration and `tests/system/README.md` for the process-level scenarios and their limits.

## Desktop

The Tauri application retains native owner setup and local HTTPS integration. Mounted forms or IPC tests do not qualify a real desktop GUI, OS credential provider, trust installation, or multi-profile offline restart. See `apps/app/src-tauri/NATIVE-SETUP.md` for its integration contract.

## Target migration

The accepted architecture uses shared Installation Devices and public `ofdb` facades only. Legacy User-owned Device transfer and internal shared-host consumption are removal work, not supported target patterns. Current support remains in `README.md`; acceptance scenarios are in `docs/goal.md`.
