# 04: Ensure DeviceRepo approve/revoke Methods Require Policy + IdP Check Before Mutation

**What to build:** The internal `DeviceRepo` methods (`approve()`, `revoke()`, `revoke_self()`) are documented/enforced to only be called after Management policy evaluation AND IdP active user check have passed. This is an internal contract enforcement.

**Blocked by:** 02 (has config gating) and 03 (has IdP check pattern)

**Status:** ready-for-agent

**Acceptance criteria:**
- [ ] `device-service/src/device_repo.rs` documents that callers must have already evaluated Management policy and verified IdP active user before calling `approve()`, `revoke()`, `revoke_self()`
- [ ] Optional: runtime checks (feature-gated) that log or panic if called without prior validation
- [ ] The primary gating remains at the API route level; this is a defensive internal contract

**Blocked by:** 02, 03
