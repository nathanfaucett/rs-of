# Unified server boundaries

## Notes

- [Specification](spec.md). This map records the security ticket; sibling work remains independently tracked.

## Decisions-so-far

- [01 — Security synchronization and admission ownership](issues/01-security-ownership.md): resolved. Device owns security replication/bootstrap and purpose-specific admission, IdP retains signer/approval/import journals behind narrow owner interfaces, and Unified constructs/registers owners. Shared SQL stream framing belongs to `db`. Focused owner and recovery tests pass; broad powerset and complexity checks timed out, and unrelated Storage test Clippy errors remain.

## Fog

- Complete broad qualification after integration; do not treat focused results as a workspace-wide pass.
- Provisioning/configuration ownership is outside ticket 01 and remains with the sibling tickets.
