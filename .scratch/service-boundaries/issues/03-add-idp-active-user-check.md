# 03: Add IdP Active User Check to Device Approval/Revocation Flow

**What to build:** Device approval/revocation now enforces the three-layer model: (1) Management policy evaluation, (2) IdP active user check, (3) Device-service state mutation. All three must pass for the operation to succeed.

**Blocked by:** 01 (needs the permission actions; this ticket adds the IdP layer)

**Status:** ready-for-agent

**Acceptance criteria:**
- [ ] Device-service repo methods (`approve()`, `revoke()`, `revoke_self()`) include a precondition check that the actor is an active user via IdP-service (`LocalIdentity::is_active_user(owner_subject)`)
- [ ] If the user is blocked or nil, the operation returns `ErrorCode::AccessDenied`
- [ ] The three-layer model is enforced at the API route level as well as the repo level

**Blocked by:** 01
