# 05: Restore Role and Permission administration

**What to build:** An authorized administrator can manage Roles and Permissions, connect them, and inspect assigned User Roles. Unauthorized users receive an intentional explanation instead of broken forms.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [x] Authorized list and create operations for Roles and Permissions succeed through the existing contracts.
- [x] A Permission can be assigned to a Role, and a Role assigned through the existing public workflow appears on User Detail.
- [x] Unauthorized access has a clear read-only or recovery state without raw `ResponseError` text or repeated failing requests.
- [x] Valid drafts remain after failed submissions, and unrelated successful saves do not emit role-load error notifications.
- [x] The authorization root cause is covered by real-owner tests, and the complete path is covered by real-backend browser tests.

## Comments

Fresh setup provisions the operator Client scope and explicit Installation policy grants. Existing policy and Client registrations are not migrated. Application-delegated grants retain their original scope.
