# Backend deployment boundaries

## Local composition

`unified-server local` composes the four owner routers over one embedded database.
It seeds local test identities without the previous unified setup host, admission
capability, provisioning workflow, or mesh endpoint. HTTPS uses a retained
self-signed leaf certificate, not the independent setup CA or OS trust changes.
See the [local backend commands](../README.md#local-development-backend).

This mode is for loopback development. It does not establish Installation
membership or qualify production Node setup. Owner files outside `unified.redb`
remain necessary for restart and resource persistence. Back up the entire data
directory, not just the database.

## Independent deployments

The independent IdP, Device, and Storage servers retain their setup, admission,
Create/Join, reset, and TLS paths. Management consumes identity policy and has no
separate setup owner. The `setup` crate must remain unchanged for these paths.

`unified-server` has no direct `setup` dependency, but its owner server dependencies
still require it. `server-common` also depends on it and re-exports independent
setup/TLS types and shared OpenAPI helpers. Removing Management's unused direct
`setup` dependency does not remove these transitive edges.

Therefore `cargo tree -p unified-server` still includes `setup`, even though the
local host no longer runs the legacy unified lifecycle. Complete graph exclusion
requires a separately scoped owner/common module or feature split. Simply removing
those dependencies or exports breaks independent deployments. Cargo workspace
exclusion would not remove a required path dependency from the graph.

Independent setup contracts remain in the OpenAPI export for independent clients.
They are not evidence that the local host exposes setup routes.

## Removed legacy state and crates

Shared contracts and models now live in `of-model`. The legacy `api`, `idp-model`,
`storage-model`, and `management-model` crates and their workspace entries are
absent. The old unified host modules and tracked `data/unified` files are absent.

This cleanup removes only tracked obsolete files. It does not delete ignored,
untracked, or user-owned data and does not migrate the previous unified layout.
Use a separate data directory for local mode.

## Qualification

The dependency audit is not runtime qualification. See
[ticket 06 validation](../.scratch/backend-simplification/cleanup-validation.md)
for commands, results, and remaining acceptance gaps. The actual four-owner
startup and independent deployment checks must pass before cleanup is resolved.
