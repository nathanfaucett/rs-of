# 01: Add DevicesApprove/DevicesRevoke IdentityActions to Management Permission Model

**What to build:** The management-service now has `IdentityAction::DevicesApprove` and `IdentityAction::DevicesRevoke` actions that can be evaluated via `LocalPermissions::evaluate()`. This is the policy gate for all device approval/revocation operations.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Acceptance criteria:**

- [ ] `IdentityAction::DevicesApprove` and `IdentityAction::DevicesRevoke` are defined in the management-service permission model
- [ ] `ManagementService::evaluate()` handles these new actions and returns `true` when policy permits
- [ ] The actions are exported from the management-service crate root
