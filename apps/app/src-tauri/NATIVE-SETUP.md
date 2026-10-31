# Native owner setup handoff

Native owns one retained local HTTPS endpoint/CA, trust consent/elevation, startup,
shutdown and explicit protected local maintenance. Unified composes the existing
IdP, Device and Storage owner workers, not a second host lifecycle. Management
participates from configuration and has no setup code/worker/operator step.

## Frontend integration contract

- `get_localhost_server_base_url` returns the verified bundled origin. Apply each
  of `/idp`, `/management`, `/device`, `/storage` once. Never substitute bundled
  bases for selected remote services.
- `get_onboarding_host` returns base/owner status or safe failure categories:
  retry, prerequisite, repair, trust_consent, trust_declined. It contains no setup
  capability. The app directly queries all three `/setup/owner/status` surfaces,
  then correlates public operation/issuer/bases and IdP `facts.login_client` before
  sign-in. No IPC status or singleton ready flag is aggregate completion.
- `get_native_setup_code({owner})` accepts idp, device or storage and returns only
  that owner's transient code (null after retirement). The client validates the
  selected bundled owner base before/after retrieval. No remote target receives
  a local code. `SetupAdmission::protected_code` owns private-state parsing.
- `IndependentSetup.svelte`/`IndependentSetupSession` drive both layouts directly
  through generated owner HTTPS operations. The same tuple reaches every owner;
  only IdP gets the administrator password. Codes/passwords are not persisted,
  logged or sent in URLs. Recovery, retry and cancellation retain exact intent;
  request disconnect is not cancellation, 202 is not completion.
- Source approval uses Device `/independent/node-joins/...` with an ordinary
  Management-audience authorized User in both layouts. No native Create/Join,
  approval, cancellation or recovery mutation proxy is provided.

## Startup and one authority

The window starts before asynchronous protected-state preparation/trust checks.
No published host projects starting; the page polls with navigation cleanup.
Remote readiness is independent of bundled startup failures. Startup reuses the
retained listener/TLS identity. Its bounded native-root OPTIONS probe accepts
200/405, not readiness-bearing GET. Only typed UnknownIssuer can request trust
installation; bind/expiry/other TLS/state errors do not authorize replacement.

Consent explains the one OS trust anchor, possible elevation and effect on other
OS-root clients. `set_local_trust_consent({consent:false})` preserves identity and
projects trust_declined; true reopens/verifies the same identity before installing
only if needed and reconstructs the verifier afterward. Failed verification is not
success. Consent is not persisted. CLI pins the CA by path without installing it;
no owner-specific hosted CA is generated. No warning bypass/plaintext fallback.

Start/retry/consent/reset/recovery share lifecycle ownership. Close cancels startup
and rejects late publication; reset joins startup/workers before retained deletion.
Owned installer processes are reaped; an unkillable elevated process must finish.
Already-made OS trust changes are not rolled back. Unix protected state, actual
provider availability and platform tools remain prerequisites, not reasons to reset.

## Persistence and owner-local reset

Preserve `<app-data>/node/listen.addr`, `idp`, `device`, `storage`, their sibling
`*.setup-host` directories and `endpoint.tls`. Each owner retains its exact
operation/worker/inventory/lock evidence. There is no host metadata SQL store,
operation journal, credential-bootstrap file or endpoint credential file. Unsupported
previous native/Unified layouts refuse intact; use exact backup/previous-binary
recovery or a new empty root, never migration or automatic regeneration.

Maintenance (`/maintenance`) is bundled-only and separate from onboarding. Explicit
RESET confirmation is required for every attempt. `reset_device` stops/joins the
composed host, then calls each owner's stopped reset entry point, preserving
completed-owner evidence for exact retry after partial failure. This sequence is
local protected management, not an HTTP reset surface or deployment broker. Each
owner's boundary is independently usable and cannot delete another owner's state.
Protected deletion precedes data deletion. Provider/state failures preserve exact
remaining evidence; no secret enumeration, shared SQL-row deletion or replacement.

`recover_reset_references` validates retained owner directories/state; it cannot
rebuild missing owner history, TLS, keys or domain data. Native reset preserves the
shared CA and does not remove installed OS anchors. Standalone owner CLI reset
instead explicitly removes only its owner CA with `--remove-os-trust`.

`pending_reset_revocations` and `revoke_reset_on_peer` retain separate public targets;
local cleanup does not acknowledge remote erasure/revocation. Authorized IdP signer
and Device revocation must both succeed before acknowledgement. Remote selection
cannot invoke local destructive controls or retrieve bundled setup capabilities.

## Qualification

See [ticket03](../../../.scratch/unified-owner-lifecycle/ticket03-validation.md) and
[ticket04](../../../.scratch/unified-owner-lifecycle/ticket04-validation.md) for exact
process/client checks and Cargo blockers. Owner contract and process suites own
lifecycle semantics; mounted forms/IPC substitutes qualify frontend wiring, not a
real Wry GUI, secure provider, OS trust installation, WAN or other platforms. No
privileged trust operation was run for ticket04.
