# Domain documentation

## Read first

Read `README.md`, `docs/goal.md`, `docs/architecture.md`, and `GLOSSARY-MAP.md`. Follow the map to each context relevant to the task.

Use canonical glossary terms in issues, designs, tests, and output. Surface conflicts instead of silently choosing a synonym or changing ownership.

## Keep document types separate

- **Glossaries:** short domain definitions without implementation details.
- **Current contracts:** behavior that callers can rely on now.
- **Target design:** accepted outcomes that are not yet fully implemented.

- **Evidence:** commands and results tied to a commit or CI run.

System topics live in `docs/`. Context topics live in `services/<context>-service/docs/` or the relevant component directory. Update current documents in place and link them from `GLOSSARY-MAP.md`.

Do not use `.scratch` or `.tmp` as stable documentation. Do not duplicate OpenAPI routes, generated API signatures, or historical status in domain documents.
