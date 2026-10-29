# 01: Put Unified endpoint keys in the secure store

**What to build:** Unified Create and Join persist endpoint private material through the existing secure-provider mechanism rather than an application-managed raw key file. Ordinary startup retains the exact endpoint identity, and explicit reset deletes the exact provider entry. Move generic provider storage beneath IdP-specific repositories only as needed to share the existing mechanism; do not introduce a new repository framework.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] Reuse existing byte-secret storage and provider selection, with generic storage independent of IdP signing-key derivation and domain ownership. Preserve appropriate feature gates and default `no_std` behavior for unaffected cryptographic code.
- [ ] Unified Create and Join durably record attempted creation before provider work and retain the exact provider reference and expected public endpoint identity. Verify stored material without assuming provider writes are create-only.
- [ ] Ordinary and native/shared startup load existing material and verify the retained public identity. Missing/corrupt material, identity mismatch, or provider failure refuses without generating replacement identity or falling back to a file.
- [ ] Preserve current endpoint binding, Device approval, and authorization behavior. Transport identity must not depend on an authenticated IdP operation or a domain database that needs the endpoint to start.
- [ ] Update all Unified endpoint existence and public-ID checks, reset, and reference recovery. Reset preserves exact references until provider deletion succeeds and supports explicit continuation after failure without deleting unrelated secrets or shared owner data.
- [ ] Delete superseded raw endpoint-key creation/read/publication code and obsolete checks. Unsupported retained layouts are refused intact before creating replacement secrets; no migration, dual-write, or compatibility fallback is added.
- [ ] Keep this slice independent of the SQL consolidation: use the current authoritative host metadata until its replacement lands, and use the replacement if already available. Do not create a second metadata mechanism to bridge tickets.
- [ ] Extend existing endpoint lifecycle and injected-provider tests for fresh creation, stable reopen, attempted-creation interruption, missing/corrupt secrets, provider errors, identity mismatch, and interrupted exact reset. Run affected independent Device/IdP provider regressions when shared code changes.
- [ ] Use isolated temporary roots and injected providers, not real secret stores or OS trust. Update affected security, operations, and native lifecycle documentation and remove obsolete behavior in this ticket.
- [ ] Record focused test/check results and limits. No full cargo-hack feature-powerset run or CRAP testing is required.
