# Work tracking

Use GitHub Issues for durable tasks, milestones, dependencies, review decisions, and status. Link each task to a milestone in `docs/goal.md` when it advances the product path.

A task should state:

- User outcome.
- Scope and non-goals.
- Acceptance checks.
- Dependencies.
- Current evidence or qualification limits.

Use the canonical labels in `triage-labels.md`. Keep triage state separate from work status such as open, in progress, blocked, and resolved.

## Temporary agent work

`.scratch/<feature>/` may contain a temporary specification, implementation notes, or local evidence. It is not the durable issue tracker and stable documentation must not link to it.

When a temporary specification is useful, keep it short and use separate fields:

```text
Triage: ready-for-agent
Work: open
Blocked by: <issue references>
```

Delete or archive temporary material when its durable decisions and evidence have been moved to the appropriate issue, current topic document, ADR, or CI record.
