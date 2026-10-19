# Setup worker ownership and trust exchange

This stable topic describes unified/native and independent owner setup. Literal HTTP shapes live in [API contracts](api.md); [operations](operations.md) covers deployment trust/recovery and qualification. Complexity/risk/priority: high-priority security/lifecycle reconciliation. Keep one lifecycle owner per host; avoid duplicate orchestration, cross-owner stores and phase-specific browser APIs.

## Ownership and composition

The API host accepts exact authorized operations and owns work after durable admission/202, independently of request/WebView lifetime. Unified/native uses the shared `ApiSetupHost` worker, one HTTPS listener with three prefixes, one endpoint/router and coordinated shutdown. Native transfers `SetupHost` rather than seeding/enrolling owners itself. Independent owners retain their own existing listeners, lifecycle gates, stores and admission codes; there is no distributed transaction.

Open/initialize each required owner once per runtime and reuse its handles through provisioning/import, current readiness checks and serving. Stopped reopen is existing-only. IdP owns identity/token signing; Management owns Device/endpoint authority and policy; Storage owns resources and consumes current Management authorization. Process/address count creates no extra Node/Device and never justifies copying endpoint keys.

Fresh Create seeds the administrator, necessary identity/public login/signing state, Management Device/endpoint binding and explicit grants; Storage/selection start empty. It performs no peer-history or resource-content exchange. Join authenticates the pinned source/issuer and exact approved offer, imports only allowlisted public identity/policy history through its owners, and retains independent local private material. Resource synchronization remains separately selected/authorized.

## Independent credential handoff

Each actual HTTPS consumer generates stable least-privilege Client identity/secret locally, records attempted-material evidence **before** secure-provider creation, and retains the raw secret only in its provider. Missing/uncertain attempted material refuses without replacement. Bundled owner calls need no confidential service Clients or OAuth self-calls.

Each owner independently admits the same canonical operation under its own Setup code before mutation. The tuple binds purpose, nonce, exact HTTPS owner roles/bases, administrator/Node choices, issuer and Join peer. Codes are never forwarded.

1. IdP obtains the exact operation-bound public registration from the bound recipient over verified HTTPS.
2. IdP creates an outstanding single-use challenge bound to operation, recipient role, Client ID and exact registration. Worker-only initiation sends it only to that recipient's bound challenge route; no public initiation/response exposes a usable challenge.
3. The recipient checks its retained operation/registration and sends challenge, registration and its local secret directly to the bound IdP registration route. Both directions disable redirects; no credential/challenge is returned to the triggering caller/browser.
4. IdP accepts only its outstanding exact capability and consumes it before provisioning through the existing owner provisioner. Public operation/nonce equality is not authority. Reflection, substitution and replay deny. Interrupted attempts need a fresh challenge with the same retained secret/registration.

Challenges are transient and invalid after restart/cancel/retirement; Client identities/secrets stay stable. Bodies/debug/status/errors/logs must not disclose passwords, raw secrets or challenges. The browser only handles transient operator codes/password input and nonsecret operation/public facts, never infrastructure credentials.

## Retry, cancellation and completion

Durable exact binding precedes mutation/202. Recovery is capability-authorized nonsecret observation, not resume. A browser rechecks current scope and operation before retry; independent completion correlates every owner's operation/issuer/target bases, not three ready flags. Already-retired owners are observed through public evidence, not sent retired codes.

Serialize privileged registration, cancellation and finalization so one wins. Recipient callbacks release the gate for provider/network waits, then revalidate active operation, cancellation, shutdown, retirement and exact registration before recording progress. Cancellation signals work; it neither rolls back owner writes nor certifies cleanup. Accepted work survives browser disconnect; host shutdown coordinates its tasks/listener.

Preserve independent `initialized.json`/`worker.json`, exact intent, attempt fences, protected inventories and completion/retirement evidence. Missing/malformed or contradictory evidence fails intact without regeneration, migration, automatic reset or silent repair. Complete-but-unretired restart is unfinished and requires authorized exact finalization. Retired admission denies setup mutations/recovery; normal API authorization remains unchanged throughout.

Completed reopen verifies retained local owner/signing/endpoint state without peer contact or reseeding; learned revocation still denies and partitions do not create a trust lease. Explicit local reset is a separate protected operation, not a setup HTTP shortcut: stop/join owners first, retain exact protected-key references, finish provider deletion before data removal, and retain failures for retry. Reference recovery restores only exact missing backed-up manifests, never keys/domain state/admission/trust or conflicting files. See [reset operations](operations.md#persistence-reset-and-recovery).

## Evidence boundary

[Management cutover](../.scratch/ui-driven-node-bootstrap/evidence/33-management-authority-cutover.md), [independent HTTPS security](../.scratch/ui-driven-node-bootstrap/evidence/independent-join-security.md) and [web/SDK integration](../.scratch/ui-driven-node-bootstrap/evidence/35-independent-web.md) qualify their recorded seams. Isolated native roots/mock providers do not qualify OS installation/provider availability. Native shared-host qualification is ongoing; no full GUI, WAN/container/platform or full-ticket acceptance is inferred.
