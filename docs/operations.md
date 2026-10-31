# Node operations

Operate equal offline-first Nodes. IdP, Device and Storage own Installation Setup;
Management owns policy administration but has no setup capability. Both deployments
use the same owner-local lifecycle. See [API contracts](api.md),
[worker design](setup-workers.md), [security](security.md), and
[configuration](../config/README.md).

## Qualification boundary

[Ticket03](../.scratch/unified-owner-lifecycle/ticket03-validation.md) records nine
passing production-process scenarios across both layouts, including retained and
source-offline filesystem reads, residency write denial/restoration, and Tombstones.
It does not establish cross-node filesystem synchronization. Historical reports
retain their original profile and failures; they are not current-tip guarantees.
[Ticket04](../.scratch/unified-owner-lifecycle/ticket04-validation.md) records consumer
checks and the unrelated sibling manifest blocker preventing current Cargo gates.
Mock protected providers, injected trust tools and isolated loopback processes do
not certify real OS-provider persistence, OS installation, rendered GUI, WAN,
containers, non-Linux platforms, or exhaustive crash/feature matrices.

## Fresh API startup and app setup

Start the intended API hosts on genuinely fresh roots. Unified uses one verified
local HTTPS endpoint with `/idp`, `/management`, `/device`, `/storage` prefixes and
one shared local certificate authority. Each setup owner exposes
`<owner-base>/setup/owner/status`, admission, Create, Join, intent and cancel. There
is no IdP-prefixed aggregate setup operation or deployment readiness endpoint.
Version/OpenAPI discovery does not initialize owner stores; unknown routes stay
404 and normal authentication is unchanged by setup status.

The desktop installs only the single retained local setup authority it uses,
with explicit consent/elevation if needed. CLI Create/Join pins that authority by
path, without OS trust mutation. Standalone owners use their own local authorities;
arrange verified HTTPS trust for their browser and service callers. An HTTP-only
proxy backend cannot forward these HTTPS listeners; do not use certificate-warning
bypass or plaintext fallback. Native-root OPTIONS listener verification is not
readiness-bearing GET.

Retrieve each 256-bit setup code through protected local/deployment management,
never status, URLs, logs, stdout or browser persistence. Send one
`Authorization: Setup <64 lowercase hex>` header only to its issuing owner.
The app takes exact four-context bases and the three codes once, submits the same
public tuple to each setup owner, and sends the administrator password only to IdP.
Bundled desktop obtains each code internally with `get_native_setup_code({owner})`
on explicit actions. Remote selection never retrieves or forwards bundled codes.

A 202 means durably accepted, not completion. Poll every owner's current status,
correlate exact public operations/issuer/bases, then obtain the IdP public login
Client. Setup does not sign in or grant resources. Management participates from
configuration with no fourth registration step. Recovery reads retained nonsecret
intent; retry keeps the exact nonce/names/purpose/bases/peer/issuer. Cancellation
signals work but promises neither rollback nor a fresh root. Retired codes cannot
be used as login authority or to start replacement operations.

## Automatic unified CLI bootstrap

The CLI is an alternate owner-local client, not a second lifecycle. Start the
server separately; keep it running while submitting Create/Join. A built binary
requires the `cli` feature. Example for a local composed endpoint:

```sh
unified-server --config config/unified/config.yaml serve

unified-server create \
  --idp https://127.0.0.1:3000/idp \
  --management https://127.0.0.1:3000/management \
  --device https://127.0.0.1:3000/device \
  --storage https://127.0.0.1:3000/storage \
  --operation-nonce 0123456789abcdef0123456789abcdef \
  --ca-path ./data/unified/endpoint.tls/ca.pem \
  --idp-code-file ./data/unified/idp.setup-host/setup-admission/state \
  --device-code-file ./data/unified/device.setup-host/setup-admission/state \
  --storage-code-file ./data/unified/storage.setup-host/setup-admission/state \
  --administrator alice --node 'My Node' --password-file ./protected-password
```

Use the actual CA filename/path and exact verified public bases published by your
host/configuration. `--ca-path` adds a pinned root to this process only; these
commands never install certificates. The password file must satisfy the protected
owner-only/nonsymlink reader; no password argv/environment/config field exists.
Choose a fresh cryptographic 32-hex nonce for a new operation; preserve it and all
choices for an exact retry. The literal nonce above is only an example.

Join takes the same four bases, three code-file paths, nonce and CA path, plus
`--peer 'https://source.example/device#<pinned-endpoint-id>'` and `--node`; it has no
administrator/password arguments. Pin/verify the source endpoint through a trusted
channel. Every target owner authenticates discovery and retains the source issuer.
The canonical Create issuer is the IdP base. CLI admission and Management health
observation are not aggregate owner readiness; inspect every owner's status/public
facts before ordinary login. Standalone calls use the same commands and their exact
owner bases, with trust for each configured endpoint.

## Source approval and ordinary use

On the source app, sign in and use Devices → Add a device. Enable requests, inspect
and approve/reject the exact offer. Both layouts use source Device
`/independent/node-joins/acceptance`, `/pending`, and `/{id}/approve` or `/reject`.
An ordinary User bearer must have `idp-management` audience and the exact Device
pairing-update plus signer-enrollment permissions. Setup codes cannot authorize
source approval. Acceptance PUT returns 204 (read back); approval returns 202,
not target completion. Device membership, endpoint trust, signer trust and resource
permissions remain distinct. A retained completed Node reopens without its original
source; configured owner dependencies and current learned denial still apply.

Storage uses kind-specific resource audiences and exact User/Application namespaces.
Selection and Residency are local-copy policy, not global permission. Passthrough
reads need an authorized reachable Full peer and cannot write. Learned denial applies
to subsequent operations on existing connections. Resource deletion synchronizes a
Tombstone; deselection/reset does not delete another Node's resources.

## Shared control-plane databases

See [control-plane deployment](control-plane.md) for injected local query clients,
default same-machine identity sockets and explicit remote SQL configuration.
Only database owners migrate schemas. Consumer reset removes local inventory and
protected material, never shared owner rows. Outage and ambiguous commit results
are not evidence of rollback. Do not open another process's live database file,
copy private signers, invent relationship credentials, or replace unavailable owners.

## Persistence, reset and recovery

Preserve owner domain roots, each `<owner>.setup-host` directory, exclusive locks,
retained exact operation, initialization/worker evidence and protected inventories,
and the one shared TLS directory (`endpoint.tls` in the sample). Dynamic desktop
roots additionally retain `listen.addr`. The Unified Host has no host operation
journal, metadata SQL database, bootstrap credentials or endpoint credential file.
Unsupported previous layouts are refused intact. Restore exact state with the
previous binary/operator backup or choose a new empty root; no migration, dual-read,
automatic reset, adoption or secret regeneration exists.

### Desktop owner-local reset

Maintenance is separate from onboarding and available only for bundled selection.
Explicit RESET confirmation discloses local consequences; retry/repair never implies
reset. Native cancels startup, closes and joins all composed workers, then invokes
IdP, Device and Storage's stopped reset entry points independently. Its local reset
journal records which owners completed so an interruption resumes exact owner work.
This is local application shutdown/management, not a server broker or HTTP reset
surface. One owner's cleanup never consumes another's capability or deletes another's
private state. Each owner remains individually resettable at its stopped boundary.

Protected deletion precedes owner data removal. Failure preserves inventories,
exact revocation targets and remaining evidence for explicit retry; never enumerate
or guess secret references. Native reset preserves the shared local CA and installed
OS trust. Reference recovery validates retained owner directories and references;
it cannot rebuild lost keys, owner operation history, TLS or domain state. Remote
revocation is separate and requires authorized IdP signer and Device revocation;
local cleanup never acknowledges remote erasure. See [native handoff](../apps/app/src-tauri/NATIVE-SETUP.md).

### Standalone stopped-owner reset

Stop only the selected owner, including its setup listener. Use the same operator,
root, config/environment and explicit OS trust-removal consent:

```sh
idp-server --config config/idp/config.yaml reset-independent --remove-os-trust
device-server --config config/device/config.yaml reset-independent --remove-os-trust
storage-server --config config/storage/config.yaml reset-independent --remove-os-trust
```

Run only the command for the owner you intend to reset. The existing exclusive lock
excludes live work. Reset captures exact supported current/historical inventory,
deletes protected slots first, removes only that owner's retained CA, then removes
local data/setup evidence. Hosted resets do not run this per-owner trust-removal
step against the shared authority. `setup-reset.json`/retired `setup-reset` evidence
fences interrupted cleanup; ordinary startup refuses it. Preserve it and rerun the
same exact command. Missing locks, damaged evidence and provider/trust failures deny
intact. No reset grants membership, deletes shared SQL-owner rows or erases peers.

## Backup and recovery limits

Back up retained owner/TLS/operation evidence together with the operator's supported
persistent protected provider. Filesystem snapshots cannot restore missing provider
secrets. Reference recovery never manufactures identity or workflow history. A
complete executable crash-consistent backup/recovery procedure is not qualified
here. Old authorization snapshots may predate revocation; do not routinely roll back
trusted history. Preserve rejected state and obtain explicit recovery evidence.
