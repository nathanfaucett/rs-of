# 03: Verify active-User authorization for Device approval and revocation

Status: ready-for-agent

Work status: resolved

## User outcome

A User who becomes inactive cannot approve a pairing or revoke a Device as an administrator, even if the User still has a token and a Management grant.

## Scope and compatibility

Check both user-facing approval routes (`approve_pairing` and legacy `approve_enrollment`) and administrative `revoke_device`. Reuse the Device authorization helper and the current local or authenticated remote IdP interface. Check the verified actor subject, not the legacy Device owner field. Keep proof and state rules in Device. Do not duplicate policy checks in repositories or add caller-precondition panics. Preserve current routes, permission actions, and legacy owner checks. Self-revocation is out of scope.

## Acceptance checks

- [x] With the applicable action granted and the token otherwise accepted, an inactive or missing User or a failed active-User lookup denies the operation without mutation. Invalid or nil subjects also deny access.
- [x] An active User without the exact permission is denied without mutation. An unavailable policy evaluation never authorizes mutation.
- [x] Valid signed pairing succeeds for an active authorized actor. The legacy approval route still rejects pending pairings. Administrative revocation preserves current protected-Device and last-approved-Device rules.

## Dependencies and current evidence

None. The routes already call `require_identity_permission`, which checks `actor.subject` through IdP before evaluating Management policy. Add focused route tests that let token validation succeed independently of the active-User lookup. Issue 07 adds real-owner coverage.
