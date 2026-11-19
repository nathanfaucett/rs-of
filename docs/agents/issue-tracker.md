# Issue tracker: Local Markdown

Issues and specifications for this repository live as tracked Markdown files in `.scratch/`.

## Conventions

- Use one feature directory: `.scratch/<feature-slug>/`.
- Store the specification at `.scratch/<feature-slug>/spec.md`.
- Store implementation issues as `.scratch/<feature-slug>/issues/<NN>-<slug>.md`.
- Number implementation issues from `01`; do not use one combined tickets file.
- Record triage state as `Status: <triage-label>` near the top of each specification or issue.
- Use the canonical labels in `triage-labels.md`.
- Keep triage state separate from work status such as open, in progress, blocked, and resolved.
- Append comments and conversation history under `## Comments`.
- Link work to a milestone in `docs/goal.md` when it advances the product path.

Each task should state:

- User outcome.
- Scope and non-goals.
- Acceptance checks.
- Dependencies.
- Current evidence or qualification limits.

## Publishing and reading

When a skill says to publish to the issue tracker, create the appropriate specification or issue file under `.scratch/<feature-slug>/`.

When a skill says to fetch a ticket, read the referenced Markdown file.

## Stable documentation

`.scratch` contains only the tracked local issue tracker. Use `.tmp/` for temporary analysis, generated reports, and evidence. Neither directory is stable project documentation. Stable documents must not link to either directory.
