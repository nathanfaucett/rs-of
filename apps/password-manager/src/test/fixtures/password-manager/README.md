# Synthetic password manager fixtures

These are hand-written research inputs for [personal-json-v1](../../password-manager-json.md). They are not live exports. All names, passwords, TOTP seeds, identifiers, card values and key text are synthetic. SSH material is deliberately nonfunctional. Do not enter any fixture into a real account.

- `supported.json`: five item kinds; complete Card/Identity/SSH field sets; seven URI states (inherited plus six explicit); text/hidden/boolean fields; duplicate field names; null and empty text; ordered history; Unicode, multiline text and leading zeroes; independent parent/child-named folders.
- `cases.json`: 35 named cases with expected outcomes. Start each case from a fresh deep copy of `supported.json`. Apply `patch` operations in order using JSON Pointer paths and JSON Patch add/replace/remove semantics. The patches use only object assignment/removal, array-element property assignment, and whole-array replacement. `raw` cases instead supply the literal file text. `destination` supplies repeat-import preconditions. `path` is an expected diagnostic location, never an input value to display.
- `sources.json`: 14 public first-party source URLs, pinned commit, retrieval date and SHA-256 content digests. No account URLs, tokens or credentials were retrieved. Moving help pages are cited separately in the reference document.

Expected importer behavior is a future acceptance requirement, not a test result. In particular, `round-trip` and `confirm-import-as-new-or-cancel` require the real application and persistence; inspecting these fixture files cannot prove them. For each blocked case, assert that the destination is unchanged and diagnostics do not contain synthetic secret values. Add storage-failure/rollback cases at the approved application/public-vault boundaries when the importer exists.

## Research validation evidence

On 2026-11-05, Python standard-library checks parsed all JSON files, checked all five type numbers, complete Card/Identity/SSH key sets, all seven URI settings, the three supported custom variants, Unicode/multiline values, leading zeroes, folder references, unique case names, and the target of each patch operation. All 14 pinned source URLs were downloaded successfully and hashed. Relative links were checked. `git diff --check` checks whitespace.

These are structural checks only, not an importer implementation, reference-client import, or application test suite. The first link check found this README had not yet been created; it was added before final validation. No Rust/app code changed, so Rust feature-power-set tests, complexity scoring and application tests are not used as evidence for this research ticket.

## Browser evidence

Playwriter session 11, tab group `baseline`: extension connected, then target-filtered enumeration returned `[]`. A second unfiltered origin-only enumeration also returned `[]`. No existing enabled tab was available. No replacement tab, browser, login, credential read, copy/reveal/export, page snapshot, screenshot, console-log read, or storage inspection occurred. The selected reference's `Vaultwarden Web 2026.6.4` label is inherited from the approved spec, not a new observation. Deployed server/client configuration and unlocked workflows remain unverified.
