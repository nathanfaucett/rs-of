# Setup worker ownership and trust exchange

One Installation Setup lifecycle serves both deployment topologies. IdP, Device,
and Storage are the three setup owners. Management participates from the supplied
configuration; it has no fourth code, setup worker, or operator registration step.
See [literal API contracts](api.md), [operations](operations.md), and [security](security.md).

## Lifecycle ownership

Each owner admits, durably binds, executes, publishes, and retires its own exact
Create/Join operation. Unified composes those existing owner workers behind one
HTTPS listener at `/idp`, `/device`, and `/storage`; standalone owners append the
same `/setup/owner/...` surface to their configured bases. Management serves at
its configured base (`/management` in Unified). There is no deployment setup
broker, aggregate readiness endpoint, host operation journal, or host metadata
SQL database. Process boundaries do not add Nodes or Devices.

The client collects the four context bases and three codes once. The same public
operation reaches each setup owner, with exactly that owner's capability in its
Authorization header. Only IdP receives the administrator password. A durable
202 is acceptance, not completion; work survives request/browser disconnect.
Conflicts reject rather than replacing intent. Recovery observes retained exact
intent without resuming work; retry preserves nonce, names, bases, purpose, peer,
and authenticated issuer. Cancellation signals eligible work, never rollback.
Retired capabilities authorize neither login nor new setup mutations.

## Onboarding observation and recovery routing

`onboarding.ts` projects selected-target owner evidence into presentation state;
it is ephemeral client state, not another lifecycle or server readiness document.
Bundled observation obtains the verified native base/failure category and queries
all three owner surfaces directly. Remote selection never substitutes local bases
or reads bundled codes. Every status has a singleton `owners` scope. Before sign-in,
the app correlates all three current ready states and public operation/issuer/base
facts, then obtains `facts.login_client` from IdP. Three unrelated ready flags,
listener availability, admission, source approval, or historical completion are
insufficient. Setup does not authenticate the operator; ordinary sign-in follows.

| Evidence | Presentation | Action |
| --- | --- | --- |
| Bundled listener not yet published | starting | Observe |
| Awaiting-choice owners | needs_setup | Explicit Create/Join |
| Creating/joining owners | working | Observe; authorize source approval separately |
| Every current owner and exact public tuple agree | ready | Ordinary sign-in |
| Network/service failure | blocked | Retry observation |
| Trust/provider/configuration unavailable | blocked | Check prerequisites |
| Unfinished or damaged retained evidence | blocked/recovery form | Exact recovery, not replacement |

`IndependentSetupSession` and `IndependentSetup.svelte` implement that one
owner-local client procedure in both layouts. Fresh Create defaults to `My Node`;
recovery preserves the retained name. Join uses the source Device HTTPS base plus
`#<pinned-endpoint-id>`, not a bare endpoint. Source approval uses the Device
`/independent/node-joins/...` contract with an authorized Management-audience User.
Optional Device preferences and naming remain editable after sign-in.

`get_native_setup_code({owner})` is protected local management. It reads only that
bundled owner's code, validates the exact selected base before and after retrieval,
and returns null after retirement. Native state parsing belongs to
`SetupAdmission::protected_code`. Codes/passwords remain transient and absent from
URLs, logs, status, and browser persistence. Browser/remote operators supply codes;
remote requests never receive a bundled capability. Browser UI origins must be
exact HTTPS origins. Packaged WebViews use the explicitly configured
`tauri://localhost` or `http://tauri.localhost` origin, without relaxing API HTTPS
or capability checks.

## Independent credential handoff

The owner workers retain stable least-privilege consumer identity/secret locally,
record attempts before secure-provider creation, and refuse lost evidence rather
than replacing material. Consumer secrets travel directly to IdP only under an
outstanding exact operation/recipient/registration-bound single-use challenge.
Public metadata/nonce equality is not authority. Challenge consumption precedes
provisioning; restart/cancel/retirement invalidates outstanding attempts. No
credential, registration secret, or challenge passes through the frontend.
Unified reuses these same workers and supplies shared verified TLS clients; it
does not revive a parallel credential-bootstrap mechanism.

## Current readiness and retained state

Owner status rechecks current local evidence, protected capability, registrations,
Device/identity policy and Storage catalogs/receipts/configured owner services.
Failure is 503/unavailable, ready:false, not an authentication-mode change. A later
valid observation can recover without reseeding. Normal route authorization stays
active. Completed reopen is existing-only and requires no original Join source;
configured local owner dependencies are still required. Native listener verification
uses OPTIONS (200/405), not readiness-bearing GET.

Preserve each owner's setup-host directory, retained exact operation, initialization,
worker/provider inventories, locks, and completion/retirement evidence. The Unified
Host owns composition/configuration, one installation endpoint, and one shared
local TLS authority, not a second operation record. Unsupported previous host
layouts are refused intact with recovery/new-root guidance; no migration, dual read,
compatibility alias, or automatic reprovisioning exists.

## Trust and owner-local reset

Desktop installs the single retained local setup authority it uses. Hosted owners
share it and create no extra authority. CLI pins that authority with `--ca-path`
and never installs OS trust during Create/Join. Standalone owners retain their own
local setup authority; arrange verified trust for their callers.

Reset is stopped, protected local host management, not an HTTP setup shortcut.
Each owner exposes its stopped reset entry point and standalone
`reset-independent --remove-os-trust`. Native shutdown joins the composed workers,
then invokes IdP, Device and Storage resets separately, retaining completion per
owner across failure. No owner reset deletes another owner's protected material or
shared SQL-owner rows. A full desktop reset is a sequence of those local operations,
not a deployment broker or remote reset contract. Hosted resets preserve the shared
CA and do not remove installed OS trust. Standalone explicit reset removes only
that owner's CA after protected deletion succeeds. Failures preserve exact evidence;
reference recovery validates retained owner state, never synthesizes keys/history.
See [reset operations](operations.md#persistence-reset-and-recovery).

## Qualification

The [cross-layout report](../.scratch/unified-owner-lifecycle/ticket03-validation.md)
records nine completed production-process scenarios and exact limits. Consumer
checks and current external Cargo blockers are recorded in
[ticket04](../.scratch/unified-owner-lifecycle/ticket04-validation.md). Mock-provider,
injected-trust and mounted-form checks do not certify real OS installation/provider
persistence, GUI, WAN, other platforms, or cross-node filesystem synchronization.
