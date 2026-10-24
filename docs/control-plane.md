# Shared control-plane databases

Unified composes `/idp`, `/management`, `/device`, and `/storage`. One injected identity database serves IdP and Management roles, permissions, and assignments. A separate injected Device database serves Device lifecycle, endpoint trust, restrictions, whole-resource selections, and replication policy. Device tables and migrations never enter identity; identity policy tables never enter Device history.

Device and pairing, Node-join, restriction, selection, and replication HTTP use `/device`, not `/management`. Management exposes role/permission/assignment HTTP only (plus public metadata and permission evaluation). There are no old-route redirects. Device administration continues to require the existing `idp-management` actor audience and exact installation/application permissions; existing `management.device.*` and `management.replication.*` service scopes remain authorization identifiers, not route ownership.

Storage receives a Device authority callback in Unified, or an authenticated `DeviceClient` for a separate service. It opens neither control-plane database. Unknown, inactive-owner, unapproved, revoked, conflicting, missing-policy, and pending-cleanup evidence fails closed; owner transfers retain the copy-retirement fence and binding rechecks. Reset removes only the stopped consumer's local setup and protected material; it does not delete rows or files belonging to either shared SQL owner.

`device.data_dir` is the second Unified database root (`devices.redb`). Storage uses `device_api_base_uri`, `device_oauth_client_id`, `device_oauth_client_secret`, `device_service_audience`, and `device_endpoint_id`. Browser Device calls use `PUBLIC_DEVICE_BASE_URL` or the explicit Device API URL; desktop composition mounts `/device`. Existing installations require fresh setup; there is no legacy file/schema migration. Standalone query repositories use the same `db::Client` facade over local injection or remote SQL; only database owners migrate schemas and retain history/sync capabilities.

## Standalone SQL owners and consumers

Run separate domain-aware owners, with database paths **outside every HTTP consumer's `data_dir`**:

```sh
cargo run -p idp-server --bin identity-database -- --database ./identity.redb --listen 127.0.0.1:50051
cargo run -p device-server --bin device-database -- --database ./devices.redb --listen 127.0.0.1:50052
```

Each command is a long-running owner, normally managed as a separate service. Do not substitute the generic ofdb SQL CLI: it does not run these domain migrations. Keep identity and Device endpoints, files, certificates, and client credentials separate. Plaintext SQL is permitted only on literal loopback addresses. Other deployments require owner `--certificate`, `--private-key`, `--client-ca` and consumer HTTPS `endpoint`, `ca_certificate`, `client_certificate`, `client_key`. SQL client certificates grant the domain's exposed query interface, not per-table OAuth authorization; keep listeners private. Never give Storage SQL configuration or credentials.

A standalone Device configuration selects both query connections:

```yaml
data_dir: ./device-consumer
server:
  port: 3001
  prefix: /device
  setup_ui_origin: https://localhost:5173
identity_database:
  endpoint: http://127.0.0.1:50051
  deadline_ms: 10000
device_database:
  endpoint: http://127.0.0.1:50052
  deadline_ms: 10000
```

IdP and policy-only Management use that same `identity_database` section and distinct local consumer directories. IdP keeps signing private material in its local OS credential provider, never on either SQL owner. Device's explicit `identity_database_path` remains a local-owner option and cannot be combined with `identity_database`; Unified instead injects clients and starts no SQL listener. Remote Device refuses retained `devices.redb`, invalid configuration, unavailable SQL, incomplete/mixed schema or missing conflict operations; there is no local fallback or consumer migration.

### Default same-machine independent setup (Unix)

With both identity selectors unset on Device and no remote SQL configuration on IdP, independent Create/Join now uses the IdP-owned identity database through ofdb-sql's Unix-socket adapter. IdP opens/migrates `idp.redb` once, serves `identity.sock` inside its private setup-host directory, and publishes the socket path in operation-bound HTTPS setup discovery. Device connects to that socket to seed and evaluate identity policy; it never opens IdP's live redb file. The socket is restarted with the IdP host and removed on clean shutdown. The protected setup-host directory limits access to the host OS account; SQL socket access is domain-level authority, not OAuth/per-table authorization. Do not share that directory or OS account with untrusted processes.

Start IdP, Device, and Storage with their ordinary independent configuration, distinct data directories, correct HTTPS origins/prefixes, protected provider access and CA trust. Submit Create/Join to each setup host with its own code as below, without adding a SQL endpoint or identity file path. Device rediscovers the local socket on completed reopen. IdP must run alongside Device, but a joining Node needs no live source peer after its own local imports complete. Other machines, separate OS accounts without access to the private socket directory, and non-Unix platforms require explicit remote identity SQL; public discovery of a socket path grants no filesystem access. Unified continues to inject clients directly and starts no socket or gRPC listener.

Policy-only standalone Management can explicitly select this same IdP-owned Unix
socket instead of remote identity SQL:

```yaml
identity_socket: /private/node/idp.setup-host/identity.sock
identity_socket_deadline_ms: 10000
```

Use the absolute socket path published in that Node's operation-bound IdP owner
facts; the example path is illustrative, not an inferred default. Configure
**exactly one** of `identity_socket` or `identity_database`. Socket deadlines default
to 10000 ms and must be 1..=300000 ms. Startup bounds connection and identity schema
validation, then applies the same deadline to queries. Missing/unavailable sockets,
ordinary files, invalid schemas, conflicting selectors and invalid deadlines fail
closed: Management never discovers a fallback, reopens IdP's live file, or migrates
an owner schema. Start IdP and its socket before Management, including on retained
restart. The connection needs the same trusted OS account/private-directory access
as Device; it grants identity-domain SQL authority, not OAuth permission. Management
still needs its own ordinarily registered confidential IdP service relationship.
Its stateless reset validates the selector without contacting or deleting the
owner socket/database, and refuses unexpected retained consumer state. Non-Unix
hosts must use remote identity SQL.

Independent IdP permission evaluation always uses its injected identity query client, local or remote. It never sends permission checks to Device. Local Join's IdP public history includes `roles`, `permissions`, `role_permissions`, and `application_user_roles`, with explicit column/value/candidate validation. Native security sync/bootstrap transfers those tables in its separate identity-policy session, and Device history carries only Device-owned tables. Grants, revocations and offline-reopened authorization use the same identity records.

### Fresh Create, Join, attachment, and reset

1. Start both migrated SQL owners. For a new Installation these are empty domain databases, not previously initialized foreign Installations.
2. Start IdP, Device and Storage with distinct, fresh consumer directories. Configure remote identity for IdP and Device, remote Device SQL for Device, and the actual HTTPS UI origin/prefixes. Leave IdP's local signer and confidential relationship credentials unset on fresh setup. Ordinary standalone serving starts the existing protected HTTPS setup host; retain OS-provider access and explicit CA trust consent. Management is the role/permission HTTP consumer, not a fourth setup worker.
3. Submit the existing independent Create tuple to IdP, Device and Storage with each host's own protected setup code. Send the administrator password only to IdP. The existing setup workers provision their local protected material and least-privilege relationships; Device seeds identity policy through the shared identity query client and its Device through the separate Device query client. Poll all owners' current readiness; 202 is not completion. No manual internal IDs or copied relationship secrets are required.
4. For a joining remote consumer, configure the **same existing Installation's identity and Device SQL owners**. Use independent Join with the source's HTTPS Device base plus `#<pinned-endpoint-id>`; an authorized User must enable source acceptance and approve the exact offer. The target revalidates the signed approval at the source and verifies the approved Device/policy and local private signer against current shared rows. SQL credentials alone never approve membership. This attaches another runtime to shared control-plane data; it does not copy history into a fresh remote database. Import into a different owner remains the existing local-owner history operation, not a SQL query capability.
5. Restart using the same consumer configuration, directories and protected provider. Completed workers restore without replaying setup or opening private database files. A configured existing IdP signer, or Device's explicit `SERVER_IDP_OAUTH_CLIENT_ID`, `SERVER_IDP_OAUTH_CLIENT_SECRET`, `SERVER_IDP_SERVICE_AUDIENCE` relationship plus serving URLs/issuer/audience, is the alternative attachment path for an already provisioned runtime; credentials must already be valid. Do not invent fresh attachment credentials or combine competing policy sources.
6. Stop a consumer before `reset-independent --remove-os-trust`. Protected setup inventory and explicit OS trust removal still apply. A stateless Device attachment with no local state needs no SQL call to reset. A setup-managed remote Device root must be empty and nonsymlink; unexpected data is retained for owner recovery, not recursively removed. Shared rows, including the reset Device's membership, remain; revoke membership separately through authorized Device HTTP when intended. IdP's exact local-secret cleanup can require SQL metadata and refuses safely on outage. This cleanup deletes protected/local inventory only, never shared `keys` rows, including metadata-only User/Client references. Public key retirement is a separate authorized domain operation, not a consumer reset side effect.

Remote admission uses Storage's existing client-credentials token flow and authenticated Device API, with finite HTTP/SQL deadlines. Unknown/revoked/conflicted endpoints, missing policy, inactive owners and SQL/Device/IdP outage fail closed. User SQL/KV/filesystem synchronization and iroh resource protocols are unchanged.

The remote setup acceptance exercises verified HTTPS Create → ordinary login/Storage use → authorized signed Join → completed reopen → Device consumer reset with live-source rows preserved → Create IdP consumer reset with every shared key row unchanged and continued login/signing by another IdP consumer. Process admission acceptance runs separate SQL-owner, Device and Storage-probe processes and checks trusted/unknown/revoked/outage behavior through the production Storage admission adapter. Fixtures use mock protected providers/OS-trust tools and explicit 64 MiB stacks; this is not default-stack, real OS installation, GUI, WAN/container, or application mTLS deployment qualification.

Review-fix qualification also covers ordinary local Create → authorized IdP administration → Storage → reopen → reset, local fresh Join and source-offline reopen with authorized IdP administration, native bootstrap/offline reopen, and native policy grant/revoke synchronization. These are isolated HTTPS/native-database tests with mock protected providers and explicit stacks, not real OS-provider or default-stack qualification. The full feature matrix still stops on CLI's no-default Clap `std` wiring; bounded CRAP coverage compilation produced no score. The three focused bootstrap interruption tests now pass after correcting inherited stage-dependent test expectations, without loosening production admission/readiness. Local Join/source-offline reopen passed three normal and two controlled-contention reruns; the catalog request's test-only 15s budget accommodates measured 6.17–6.22s multi-owner authorization/rechecks that exceeded the inherited 5s test-client deadline. Production deadlines are unchanged; this is not a latency SLA. See the [review-fix ticket](../.scratch/shared-control-plane-databases/issues/05-review-fixes.md) for commands, evidence and limits.
