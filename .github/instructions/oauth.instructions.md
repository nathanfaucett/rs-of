---
applyTo: "**"
---

# Instructions

Follow the canonical repository guidance in `AGENTS.md` and `CONTRIBUTING.md`.

- Read current support, the product goal, target architecture, and relevant context contracts before planning.
- Do not require sibling source checkouts or manual mirror edits.
- Preserve public `ofdb-sql`/`ofdb-kv` boundaries.
- Keep Device records in Device service and authorization policy in Management.
- Make an explicit compatibility/migration decision for persisted data, public APIs, and wire protocols.
- Delete obsolete code and documentation in the same change as their replacement; do not retain parallel legacy contracts.
- Never present target architecture or unrun validation as current behavior.
