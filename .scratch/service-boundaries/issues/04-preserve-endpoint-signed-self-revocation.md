# 04: Preserve endpoint-signed self-revocation

Status: ready-for-agent

Work status: resolved

## User outcome

A Device can withdraw its own approval without User sign-in, including when IdP is unavailable.

## Scope and compatibility

Preserve the current `revoke_self` endpoint, signed payload, and Device-state rules. Verify endpoint-key possession; do not add a User token, Management grant, or cross-owner repository check. Administrative revocation is a separate operation.

## Acceptance checks

- [x] A valid signature revokes the matching Device with no bearer token and without calling IdP or Management.
- [x] A missing, invalid, or different-key signature changes no Device state. A Device cannot revoke another endpoint using its own signature.
- [x] Verify that the revoked Device is no longer available through approved endpoint lookup.

## Dependencies and current evidence

None. The handler checks the signature before calling the Device owner mutation. The Device authority test already checks valid and invalid proofs with an invalid bearer token. Extend it to cover the no-token and owner-independence requirements.
