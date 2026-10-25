# Deployment-independent system tests

## Notes

- [Spec](spec.md); [assertion dispositions and final audit](coverage.md).
- Historical validation: [02](ticket02-report.md), [03](ticket03-report.md), [04](ticket04-report.md).

## Decisions-so-far

- [05 resolved](issues/05-final-coverage-and-basic-validation.md): all 155 baseline
  declarations reconciled; nine actual-process scenarios pass; real dependency
  cleanup and generated-client health poll; focused external/CLI/provider/socket,
  live reconciliation, strict library lint and deterministic contract evidence.
  [Final report](ticket05-report.md) preserves earlier failures and exact limits.

## Fog

- Cross-layout actual-process live reconciliation lacks a public exact trigger;
  enabled private live tests retain those assertions (two pass with 64 MiB stack).
- Mixed owner-fixture relocation remains deferred; no production visibility or
  server dependency added to force it. Default-stack/concurrent deadline and
  real OS-provider/network/platform qualification are not claimed.
