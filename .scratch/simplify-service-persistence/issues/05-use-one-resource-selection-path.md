# 05: Use one scoped Resource Selection path

Status: ready-for-agent
Work: resolved
**Blocked by:** None (can start immediately).
Spec: [Simplify IdP, Management, and Storage persistence](../spec.md)

**What to build:** A User or authorized administrator manages Selection through one Device-facing path. Storage uses that Selection to authorize replication. Deselection stops future authorized replication without deleting the Resource.

- [x] Remove `sync_selections`, its Management routes, and the Storage caller that depends on them. Management still supplies administrative permissions; it does not store a second Selection.
- [x] Check Device, owner, Application, Resource kind, and Resource ID on the remaining path. Prove that permission in one scope cannot list, replace, or remove a Selection in another scope.
- [x] Use real owner routes and native synchronization to verify both Devices need approval and Selection, and that learned deselection stops later synchronization without deleting the Resource. Verify the removed routes are unavailable.
- [x] Update the initial schema, OpenAPI, generated clients, and affected consumers together. Run the real-backend browser vault regression. No migration, legacy path, or version bump.

Evidence: The ticket's owner HTTP tests rejected cross-scope Selection and confirmed removed routes return 404. Native tests passed for approval, Selection, and learned deselection; the Chromium vault tests passed before the issuer merge. Generated-client and Chromium sign-in checks passed after that merge. Final native and vault reruns timed out.

2026-11-24: All 73 Device tests and focused library Clippy passed. Full `pnpm test` and OpenAPI contract checks passed. Broader offline-convergence qualification remains outside the supported claim.
