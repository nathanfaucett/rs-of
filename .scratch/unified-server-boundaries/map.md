# Unified server boundaries map

## Notes

[Specification](spec.md) implemented on `refactor/unified-server-boundaries`. All implementation tickets are resolved. [The final responsibility audit](audit.md) records retained host behavior, extracted owner operations, test decisions, independent review, and incomplete broad qualification. Resolved implementation does not mean every feature combination or deployment is qualified.

## Decisions-so-far

- [01 — Security synchronization and admission ownership](issues/01-security-ownership.md): Device owns security replication/bootstrap and purpose-specific admission; IdP retains exact identity/signer/approval/import evidence behind owner operations. Unified constructs/registers owners. Shared bounded SQL framing belongs to `db`. Implementation and review fixes: `a45ec3390`, `d8a0e21c1`.
- [02 — Provisioning ownership](issues/02-provisioning-ownership.md): IdP owns retained credentials/signer evidence, operator-login registration, identity settings validation and database construction; Management owns retained administrator grants. Device and Storage own their existing-only preflight. Unified retains immutable host references, concrete owner-call ordering and cross-owner construction/publication acknowledgements. Initial extraction `c22007dcd`; remaining store ownership closed in `3db3c5eaa`.
- [03 — Composition audit and validation](issues/03-composition-audit-validation.md): audited remaining host modules, fixed review findings and stale owner-boundary fixtures, and reconciled stable design documentation. Three full two-host scenarios, partial-credential recovery, policy cleanup, setup route lifecycle and real SDK qualification pass. No remaining actionable production ownership violation found by independent static review.

## Fog

- Full all-feature Unified library suite is not a completed pass; the timed-out run's reproduced fixture failures were fixed and affected scenarios passed afterward.
- Required Unified feature-powerset testing and workspace `just crap` each exceeded their 600-second bounds. No complete feature matrix or CRAP report is available.
- Strict affected-library Clippy passes. Earlier all-target Clippy reports untouched Storage test fixture warnings; no all-target lint pass is claimed.
