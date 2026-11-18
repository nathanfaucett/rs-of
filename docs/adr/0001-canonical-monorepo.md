# Canonical monorepo

**Status:** accepted

`nathanfaucett/rs-of` is the sole source tree for `of`, `ofdb`, `offs`, and `ofnet`. A single canonical checkout removes manual mirror edits and makes cross-context changes reviewable and testable together. Other repositories may become generated mirrors or publication targets, but normal development and CI must not require them.

## Consequences

CI, examples, and documentation must work from this checkout alone. A change that needs another source checkout is incomplete until that dependency is represented by a local source or an explicit versioned artifact.
