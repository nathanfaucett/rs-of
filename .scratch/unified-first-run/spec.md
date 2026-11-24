# Unified first-run setup

Status: ready-for-agent

## User outcome

With `just unified` and `pnpm apps:dev`, a fresh data root opens **Set up this Node** at `https://of.localhost:1355`. No environment variables are required. There are two choices:

- **Start a new Installation**: set the first administrator username and password. Start sign-in only after the owners report ready. Do not seed a default administrator on this path.
- **Join an existing Installation**: select its HTTPS services and use its Device pairing and approval contract. The new Device proves possession of its endpoint key, and an authorized user of the existing Installation approves it. Do not create local accounts or a second server authority.

On restart, a completed setup remains selected. An incomplete setup can be resumed without replacement or reset. This work supports the Device administration path in `docs/goal.md` (M3). Server-to-server replication is out of scope.

## Scope and dependencies

- Reuse the existing first-Installation UI, owner setup lifecycle and protected setup admission. Adapt the unified composition to the Portless public origins. Do not make setup writes available without operator authorization.
- Use verified setup state for the normal browser startup path. OIDC discovery alone does not prove that the owners are ready. Keep seeded test identities in an explicit test-fixture path only.
- Reuse the Device pairing API in `openapi/device.json`. Keep user sign-in, Device approval, and Resource access separate. A user may need to sign in to the existing Installation to authorize pairing; this is not a sign-in to an unconfigured local authority.
- Preserve existing seeded data. Do not silently adopt, overwrite, or reset roots from the old local mode. Define and test explicit handling for those roots before changing the default.
- Preserve password-manager sign-in and local vault use. Verify the OAuth client configuration against the selected Installation instead of assuming the seeded client exists.

## Acceptance checks

- With a fresh root and no `PUBLIC_*` overrides, open the main app at its default Portless URL. It shows the two setup choices, not sign-in.
- Create an Installation, sign in with the entered credentials, and verify that the old default credentials are not accepted. Restart and verify that setup does not repeat.
- Join an existing test Installation. Verify that the Device is not approved until the authorized pairing exchange completes. Restart and verify that the approved Device identity persists and that no second IdP or administrator was created.
- Verify that unauthorized setup writes fail and that an incomplete setup cannot become sign-in-ready. Use real-backend browser tests for both choices.

## Current evidence

The current unified runtime uses local owner routers without the first-Installation setup lifecycle. The IdP seeds a test account and clients, and the browser's configured login path accepts OIDC discovery as ready. The desktop/independent flow has the first-administrator form. The Device API has proof-of-possession pairing; the unified first-run UI does not provide that flow yet.

## Comments

The maintainer confirmed that joining means Device approval, not adding a server authority.
