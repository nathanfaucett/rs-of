# Independent setup trust exchange — implementation design

Source base: bba9d4921. Recorded before implementation; not acceptance of tickets 02/03.

## Authority and retained ownership

Every owner independently admits the same canonical exact operation with its OWN Setup code before mutation. The operation binds purpose, exact HTTPS owner roles/bases, canonical issuer (authenticated discovery for Join), trusted peer, administrator/Node choices. Codes are never forwarded. Topology creates no Node or Device per process; IdP owns token signing, Management owns actual transport identity/policy, Storage owns only its resources and service bindings.

## Credential handoff

Each actual consumer generates its own stable least-privilege Client ID and random secret. The raw secret is retained only in its secure provider, with durable attempted-material evidence persisted BEFORE provider creation. Missing/uncertain attempted material refuses without replacement. Use existing RawKeyringRepo; do not invent verifier import or cryptography. IdP reuses OwnerProvisioner.ensure_setup_infrastructure_client, which accepts plaintext transiently and persists its existing verifier/metadata-root contract.

The IdP worker obtains the exact public registration from the operation-bound recipient over verified HTTPS. It creates a random single-use challenge bound to the full operation, recipient role, Client ID and exact public registration. It sends the challenge ONLY to the bound recipient HTTPS route, with redirects disabled. No public challenge-initiation route or response reveals a usable challenge. The recipient checks its independently retained operation and exact registration, then POSTs challenge + registration + secret ONLY to its bound IdP HTTPS route, also without redirects. It never returns the secret/challenge to the triggering caller. IdP accepts only an outstanding exact challenge; nonce or public operation equality alone is not authentication. Consume before provisioning; failed/interrupted attempts require a fresh challenge and exact constructor retry with the same consumer secret. Replay, role reflection, registration substitution and mismatched operations deny.

Challenges are transient and invalid after restart/cancellation; the secret/Client registration stays stable. Hold the owner lifecycle serialization gate through accepted registration, cancellation and finalization so one wins. Retirement denies new exchanges and recovery; completed-but-unretired state requires exact authorized finalization. Bodies/debug/error/status/logs must not disclose raw secrets, passwords or challenges. Browser sees public status/nonsecret recovery only.

## Lifecycle and transport

Host-owned worker acceptance is 202 only after durable exact binding, not completion. The worker owns cancellation independently of request/browser lifetime. A single existing TLS listener dispatches setup and normal routers; reuse the same owner-local opened store through seed/import/serving. No cross-owner opens or copied endpoint keys. Normal protected route authorization stays unchanged. IdP OAuth serving must not require its signer binding endpoint to equal a separate injected transport endpoint; cryptographic signer validation remains required. Join public identity/policy import is separate from Create and resource sync, with existing approval/history validation preserved.

## Qualification seams and risk

Use the spec-agreed production HTTPS setup and normal owner APIs, plus stopped/offline reopen. Red/green in vertical slices. Main risks: competing setup callers, challenge disclosure/substitution/reflection/replay, uncertain provider writes, cancellation/finalization races, partial readiness and repeated store initialization. High priority, high security/lifecycle risk; avoid a generic framework or per-phase app routes. Isolated native roots/mock providers only. No powerset/CRAP, deadline increases or real trust/provider mutations.
