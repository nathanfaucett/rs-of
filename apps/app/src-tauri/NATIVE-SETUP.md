# Native/shared setup handoff

Native owns OS trust/elevation consent, the bundled TLS listener, local host observation, coordinated shutdown, and explicit local reset. `native_host.rs` transfers `SetupHost` into the same `ApiSetupHost` worker used by the HTTPS unified host. There is no native administrator seeding, issuer discovery, enrollment, owner-store composition, or separate Create/Join runtime.

## Frontend integration contract

No TypeScript/Svelte is changed in this slice. The frontend owner must replace the obsolete native mutation branches with its shared HTTPS setup client:

- `get_localhost_server_base_url` returns the bundled verified HTTPS origin. Apply `/idp`, `/management`, `/storage` once. Only use this origin when the operator selects bundled services; selecting remote API bases must not call native Create/Join or substitute the local bases.
- `get_native_setup_code` is explicit **protected local host management**, returning the transient local setup capability or `null` after retirement. Never send it to a selected remote host, put it in a URL, persist it in frontend storage, or log it. This is not an HTTP status endpoint. Remote hosts retain their own admission and deployment-managed trust prerequisites.
- Use the selected host's normal `/idp/setup/status`, `/admission`, `/create`, `/join`, `/intent`, `/cancel` contracts. Accepted work belongs to the host, not the invoking request or WebView. Source approval uses the existing authenticated owner HTTPS routes, not native commands.
- `get_native_host_error` returns a local prerequisite/startup failure for display. Failed native startup leaves the UI available for explicit runtime retry/reset/reference recovery; it does not regenerate host/TLS/provider state.
- `retry_local_runtime`, `reset_device`, `recover_reset_references`, `pending_reset_revocations`, `revoke_reset_on_peer`, `get_setup_stage`, `get_desktop_client_id`, and `complete_device_setup` remain native capabilities. Device completion is bound to the verified public login Client and does not enable API authentication or establish Node readiness.

Removed Rust commands: `provision_initial_administrator`, `join_installation`, `cancel_installation_setup`, `get_setup_intent`, `node_join_accepting`, `pending_node_joins`, `approve_pending_node_join`, `reject_pending_node_join`. There are no compatibility wrappers. The old `/lidp/setup/status` probe and native `/trust` HTML route are also removed; use the shared `/idp/setup/status` contract.

The native router selects one exact bundled UI Origin: `https://idp-desktop.localhost:1355` under Tauri's build-script `dev` configuration (matching `tauri.conf.json`), otherwise the platform's Tauri Origin. The shared worker enforces that Origin and setup admission independently. TLS verification clients are constructed **after** OS trust installation. No custom CA is added to a production client.

## Persistence and reset

The shared host owns `<app-data>/node/{host.json,setup-admission,idp,management,storage,endpoint,endpoint.credentials,endpoint.provisioning,endpoint.tls}`. Its durable intent, attempt fences, protected inventories, admission retirement and owner runtime are authoritative. Old native `setup-ids.json`/`node-setup.json`/`services` state is not migrated or treated as fresh; startup refuses it intact and reports the need for explicit independent recovery.

Reset closes and joins the native listener/shared worker before taking the stopped host guard. It captures exact nonsecret protected-key references and private manifest backups before deletion. Owner APIs validate the exact original Create/Join journal and secret inventory; missing owner metadata or provider failure leaves the owner data, journal and references intact. A durable `data-pending` phase prevents repeating provider work after successful key deletion. Resumed data cleanup coordinates the retained host lock when still present. Public revocation targets survive reset; both IdP signer and Management Device revocation require their ordinary authorized User audience exchanges before acknowledgement.

Explicit reference recovery restores only missing exact backed-up manifests, never conflicting files, endpoint keys, domain data, trust, or setup admission. Lost/malformed provider inventories are not guessed. Reset does not execute OS CA removal; independently installed public CA anchors remain an explicit host-administration operation. No real profile, CA store or secure provider was modified during implementation.

Complexity/risk/priority: high-priority security/lifecycle reconciliation; reduce complexity by deleting duplicate orchestration and retaining one shared worker. Critical risks are wrong host selection, competing lifecycle owners, premature completion, lost identity evidence and deleting data before protected-key cleanup. Tests exercise these seams rather than a platform/feature matrix.

## Validation boundary

Focused tests added for shared native HTTPS routing/admission/Origin, one listener, retained-port reopen, trust/verification refusal, host/TLS evidence loss, explicit reset retry/recovery, retained Join references, two-owner peer revocation, and mock-provider Create/completed retry/offline reopen/reset. The provider test runs in a bounded child process with an isolated in-memory provider and test-only native certificate roots; the trust tests inject commands. They do not qualify real OS trust installation, external provider availability, physical peer discovery, process-exit UI behavior, rendered desktop Create/Join, or independent frontend wiring.

Rust build/test execution is pending the resource/build token. Do not treat source formatting/static checks as passing Rust tests. Once granted, run one bounded command at a time:

```sh
CARGO_BUILD_JOBS=1 cargo test -p lidp native_host::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 cargo test -p lidp native_shared_create_reopen_and_provider_safe_reset -- --nocapture --test-threads=1
CARGO_BUILD_JOBS=1 cargo test -p lidp setup_reset::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 cargo test -p lidp setup_peer_revocation::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 cargo test -p lidp localhost_trust::tests -- --test-threads=1
```

The existing unified production Join tests remain the qualification seam for authenticated peer approval/import/offline reopen. This native slice delegates to that worker unchanged; it does not claim a new rendered GUI or physical Join qualification.
