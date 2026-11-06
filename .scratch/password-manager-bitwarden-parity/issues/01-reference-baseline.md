# 01: Verify the reference feature baseline

**What to build:** An evidence-backed comparison of personal-vault workflows and supported data fields that later tickets can use without guessing.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

**Type:** research

**Work status:** resolved

Tracker Status: resolved

- [x] Compare the existing local application with the selected Vaultwarden web vault using Playwriter. Reuse existing tabs; do not open replacement tabs or enter passwords. Record locked-session limits instead of bypassing them. (Attempted; no enabled tabs were available. Limit recorded, not bypassed.)
- [x] Do not reveal, copy, export, change, or log real credentials or live vault data. Use official documentation and synthetic examples where account inspection is unsafe or unavailable.
- [x] Record reference version, client/deployment restrictions, evidence sources, observed behavior, and unverified claims in a stable named topic document.
- [x] Define supported fields and validation semantics for Login, Card, Identity, Secure Note, and SSH Key; URI settings, custom field variants, history, folders, Trash, and JSON portability. Distinguish observation from proposed product behavior.
- [x] Assign each candidate feature a disposition: core, follow-on, unsupported, or unverified. Do not equate local vaults with organizations or stored URI rules with autofill.
- [x] Resolve enough of the supported JSON subset for later tickets to proceed using synthetic fixtures. Explicitly list remaining limits; do not claim full parity or invent delivery estimates.

## Answer

The bounded reference baseline is complete. See [reference comparison](../../../docs/password-manager-reference.md), [five-type field and JSON contract](../../../docs/password-manager-json.md), and [synthetic fixtures/evidence](../../../docs/fixtures/password-manager/README.md).

The documents separate inherited observations, first-party documentation/source, and proposed local behavior. They define all five field sets, URI/default semantics, custom variants, dates/history, folders, Trash, feature dispositions, unsupported-data handling, duplicate policy, and logical round-trip scope. Fourteen first-party source files are pinned and hashed. No application code, shared map, glossary map, or live vault data changed.

Browser limit: Playwriter connected but both enumerations returned no enabled pages. No new tabs, credentials, live exports or account changes were used. The earlier `Vaultwarden Web 2026.6.4` label is attributed to the spec, not claimed as freshly observed. Selected server version/configuration and unlocked workflows remain unverified. This blocks observed deployment parity, not the supported-subset design.

Validation: Python structural checks passed for JSON syntax, five types/complete field sets, URI/custom variants, Unicode/multiline/leading-zero preservation, folder references, 35 unique case definitions and all patch targets, 14 pinned source records, and relative document links. `git diff --check` passed. The first link check found the pending fixture README; it was added and the check then passed. Importer round-trip and application/Rust tests were not run: no implementation changed. Those remain later-ticket acceptance gates.

## Comments

- 2026-11-05: Claimed on `implement/password-manager-01` in the assigned worktree. Integration ancestry confirmed at `42391ef3c`. Triage remains `ready-for-agent`. Scope: stable reference documentation and synthetic evidence only; no application changes. Complexity: moderate; main risk is exposing live data or overstating parity. No background-agent tool is available, so primary-source research will run here.
- 2026-11-05: Resolved with documentation and synthetic evidence. Triage remains `ready-for-agent`; work/tracker status is `resolved`. Shared map updates are left to the merger, as requested. No full-parity claim or delivery estimate.
