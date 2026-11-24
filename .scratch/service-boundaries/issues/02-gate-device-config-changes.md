# 02: Gate Device Config Changes Behind Management Policy Evaluation

**What to build:** Device-service API routes that mutate device configuration (e.g., rename, transfer ownership) now require Management policy evaluation before allowing the change. Clients must present valid Management permissions to change device name or address.

**Blocked by:** 01 (needs DevicesApprove/DevicesRevoke actions for policy evaluation)

**Status:** ready-for-agent

**Acceptance criteria:**
- [ ] Device-service routes for device config changes (e.g., `transfer_device`, `update_device`) call Management policy evaluation before mutating device state
- [ ] If policy evaluation fails, the route returns `ErrorCode::AccessDenied`
- [ ] Device-service no longer accepts config changes without Management permission

**Blocked by:** 01
