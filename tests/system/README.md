# Shared Node Create qualification

Run from the repository root on Linux, with the sibling `ofdb`, `ofnet`, and
`offs` checkouts required by the workspace:

```sh
cargo build -p idp-unified --example system-test-service -p idp-server --bin identity-database
cargo test -p idp-unified --test system_create -- --ignored --test-threads=1 --nocapture
```

Use the same Cargo target directory/profile for both commands. These expensive,
process-launching acceptance tests are explicitly opt-in, not silently skipped
qualification. Each test can also be selected by its exact name:
`unified_create_and_use` or `independent_create_and_use`.

## Public seam and concrete deployments

`create::scenario` is the single assertion body. It checks generated-client
Installation discovery, ordinary Initial Administrator login and User info,
Device/Management database health, authorized Storage filesystem creation/read,
and anonymous denial of that same resource. It never opens a domain database.
All HTTP operations use generated clients and models; there are no raw HTTP
fallbacks. The common independent setup contract is consumed through its
generated IdP client, including when addressing Device and Storage workers.
Ordinary owner operations use the respective generated owner clients.

- **Unified:** one child calls the production Unified CLI `run`, starts a fresh
  neutral loopback HTTP host, admits Create through its protected host code,
  polls public current readiness, and verifies the exact completed Create to
  obtain its public operator-login Client. TLS termination at a deployment proxy
  is outside this loopback CLI qualification.
- **Independent:** actual production CLI entry points run in separate OS
  processes for IdP, Device, Storage, and Management. The actual
  `identity-database` executable is a fifth process. Its database is outside all
  consumer roots; IdP/Device/Management use its explicit loopback SQL connection.
  Device owns its separate local Device database. Storage receives no SQL access.
  IdP, Device, and Storage each admit the correlated Create tuple with their own
  protected host code. Only IdP receives the password. Completion requires each
  current public readiness response and matching public operation, issuer,
  service bases, and nonempty endpoint evidence, not just an accepted request.
  Management is not a setup worker: an ordinarily authorized administrator
  registers its Application and confidential service Client through generated
  IdP operations, then starts the ordinary Management CLI with that relationship.

Every address is explicit and uses a separately allocated ephemeral loopback
port in the independent layout. IdP uses no prefix, Device uses `/device`,
Storage uses `/storage`, and Management uses `/policy`; there is no common-origin
or common-prefix derivation. Independent setup supports only empty or owner
composition prefixes; Storage's resource router uses `/storage`. Independent
Create's Canonical Issuer is its exact IdP base under the supported contract;
Unified retains its separately generated logical Installation issuer.

## Safety and limits

The test-only example installs the existing mock credential store (ignoring
provider-specific entry modifiers, as existing provider fixtures do), then calls
production service entry points. It does not construct replacement routers,
provision domain state, or add production test hooks. Each process has its own
in-memory provider: **retained-process restart is not qualified by this fixture**.
Workers use verified HTTPS and an explicit isolated CA bundle, also supplied as
native trust to child processes. Test-only `trust`/`pkexec` commands prevent real
OS trust installation. Management's production CLI exposes loopback HTTP; no
external TLS proxy is introduced or qualified here. The SQL listener is also
literal-loopback plaintext, as allowed by its production CLI.

Roots are private and ephemeral; configuration credential environment variables
are removed before explicit fixture settings are applied. There are bounded
connect/request/poll deadlines, two runtime workers per service, and explicit
64 MiB service stacks. Independent IdP uses the existing small test password
configuration (1 MiB, one iteration), not production password-cost qualification. Child stderr is drained while retaining at most 16 KiB per
service in the private root; stdout and stdin are disconnected. Tokens, password
responses, Client registration responses and setup capabilities are never
printed. Test failure retains the private root and reports only its path. Success
removes it. Owned children are killed and reaped before their roots are removed,
including unwinding. No unrelated process is signalled. A forcibly killed test
runner is not an orderly teardown test.

This is same-machine Linux mock-provider qualification, not real OS-provider or
trust installation, default-stack, proxy/mTLS, WAN/container, restart, Join,
recovery, synchronization, or full-suite certification. Existing Unified tests
remain owned by ticket01; do not delete their Create coverage based only on this
narrow replacement. See the ticket02 report for exact validation and gaps.
