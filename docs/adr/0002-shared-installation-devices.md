# Shared Installation Devices

**Status:** accepted

A Device is an approved Installation endpoint that may hold authorized replicas for multiple Users and Applications. Device service owns Device identity, approval, revocation, and configuration; Management authorizes administration; Resources remain in `(User subject, Application)` namespaces; Selection maps a Resource to a Device. This replaces the target need for a single User owner and Device ownership transfer.

## Consequences

The legacy `ownerSubject` and Device-transfer model must be removed as the implementation migrates. Revocation and deselection still control later use when learned, but neither transfers Resources nor guarantees remote erasure. Until migration and end-to-end tests complete, shared multi-User Device behavior is target design rather than current support.
