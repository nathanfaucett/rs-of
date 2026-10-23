# Unified server boundaries map

## Notes

The spec spans provisioning, security protocols, and parent host/configuration/runtime composition. A resolved ticket does not imply the entire spec or broad integration qualification is complete.

## Decisions-so-far

- [02 — Provisioning ownership](issues/02-provisioning-ownership.md): IdP interprets retained credentials and signer evidence and constructs operator login registrations; Management owns the retained administrator grant; Storage owns acknowledged-root checks. Unified retains concrete call ordering, immutable process references, and cross-owner construction/publication acknowledgements. Retained journal formats and focused retry/recovery/offline Create coverage pass. Implementation: `c22007dcd`.

## Fog

- Device needs an existing-only provisioning preflight to remove raw retained `devices.redb` checks from local Create/Join without allowing replacement stores.
- Full-node Join qualification currently stops in an unchanged fixture querying Management tables in the Device store; not baseline-qualified or fixed by ticket 02.
- Security protocol extraction, parent host/configuration/runtime audit, shared integration reconciliation, feature-powerset testing, and complexity/coverage qualification remain outstanding.
