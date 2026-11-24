# 02: Harden local Markdown tracker setup in the global skill

**What to build:** The global setup skill reliably configures a durable local Markdown tracker on the first run. It detects existing tracker guidance, ignored tracker directories, and generated artifacts before proposing changes. Its confirmation draft covers tracking, artifact separation, and conflict removal, and its completion checks prove that issues are visible to Git and temporary artifacts remain separate.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

Work: resolved

- [x] Exploration reads existing agent configuration, ignore rules, contributor guidance, and tracker contents before proposing a tracker.
- [x] Existing local Markdown configuration takes precedence over remote-host defaults; no GitHub tracker is selected merely because a GitHub remote exists.
- [x] Local Markdown setup identifies an ignored tracker directory and includes the required tracking correction in the confirmation draft.
- [x] Generated artifacts already inside the tracker directory are identified and moved to a confirmed temporary location with related references updated.
- [x] Conflicting tracker guidance is included in the draft and replaced with one consistent configuration after approval.
- [x] Completion verification proves tracker files are visible to Git, temporary artifacts are ignored, and contradictory guidance is absent.
- [x] A representative setup scenario covering a GitHub remote, existing local configuration, an ignored tracker, conflicting guidance, and generated artifacts validates the complete outcome.
- [x] The already configured repository is not reconfigured, and no new runtime or repository dependency is introduced.

## Comments

2026-11-24: The existing global-skill resolution remains accepted. Tracker files remain visible to Git and `.tmp` remains ignored. No repository tracker reconfiguration or new dependency was made; the global representative scenario was not rerun in this pass.
