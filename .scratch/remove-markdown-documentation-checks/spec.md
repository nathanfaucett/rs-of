# Remove Markdown documentation checks and harden local tracker setup

Status: ready-for-agent
Work: open
Milestone: M1 — Canonical foundation
Dependencies: None identified.

## Problem Statement

The repository contains a Markdown local-link and anchor checker that the maintainer does not want to retain. Its command entry points, CI job, dependency edges, contributor instructions, milestone language, and tracker acceptance criteria keep it active even if its script alone is removed. All of those references and usages must disappear together.

The earlier local tracker setup exposed a separate residual issue in the global setup skill. The repository now correctly tracks its local Markdown tracker and separates temporary artifacts, but the global skill does not reliably detect existing tracker configuration, ignored tracker directories, conflicting guidance, or pre-existing generated artifacts. Future runs can repeat the original mistake.

Previously addressed retro items concerning unavailable skill dependencies, interaction wording, and dirty-worktree commit handling need no further work in this specification.

## Solution

Remove the Markdown documentation checker as a complete facility rather than replacing it. Delete its implementation, command aliases, help text, CI job, CI dependency references, contributor commands, automatic-link-check milestone statement, and acceptance criteria in tracked specifications and tickets. Preserve unrelated validation, including whitespace checks, Rust and TypeScript checks, browser tests, synchronization tests, and other product qualification.

Update the global tracker setup skill so local Markdown setup inspects existing configuration and repository state before writing. The skill must ensure the tracker is visible to Git, identify and resolve conflicting tracker guidance, and separate generated temporary artifacts from durable tracked issues. It must include these changes in its confirmation draft and verify the result.

Do not repeat the already completed repository tracker setup. Do not alter unrelated CI workflow work or introduce a replacement documentation checker.

## User Stories

1. As a maintainer, I want the unwanted Markdown checker implementation deleted, so that the repository no longer carries a facility I do not use.
2. As a contributor, I want obsolete documentation-check commands removed from development instructions, so that I cannot invoke a deleted script through documented commands.
3. As a contributor, I want Just and package command listings to contain only available commands, so that command discovery remains accurate.
4. As a maintainer, I want the CI documentation job and its dependency edges removed, so that CI no longer runs or requires the deleted facility.
5. As a maintainer, I want milestone language to stop promising automatic documentation-link checks, so that target documentation matches the accepted tooling policy.
6. As an implementation agent, I want tracker acceptance criteria to stop requiring the deleted checks, so that completed work is not blocked by an impossible or unwanted qualification.
7. As a reviewer, I want unrelated behavioral and qualification checks preserved, so that removing the Markdown checker does not reduce product test coverage.
8. As a repository maintainer, I want unrelated working-tree and CI workflow changes preserved, so that this cleanup does not absorb or revert separate work.
9. As a user of the global setup skill, I want existing tracker configuration read before a tracker is proposed, so that local Markdown setups are not mistaken for GitHub setups.
10. As a user of the global setup skill, I want local tracker directories to be visible to Git, so that durable specifications and tickets can actually be committed.
11. As a user of the global setup skill, I want generated artifacts separated from the tracker during setup, so that durable work and temporary output have distinct storage policies.
12. As a user of the global setup skill, I want conflicting tracker guidance included in the setup draft, so that I can approve one consistent configuration before files are changed.
13. As a future agent, I want setup completion checks to prove the tracker is unignored and guidance is consistent, so that I can reliably publish to the configured tracker.
14. As a maintainer, I want no replacement Markdown framework or new dependency, so that this remains a deletion and setup-skill correction rather than another tooling expansion.

## Implementation Decisions

- Treat repository cleanup and global skill correction as two scopes. Do not repeat the repository tracker setup that is already complete.
- Delete the Markdown link-and-anchor checker implementation rather than repairing its treatment of tracked issue files.
- Remove every public invocation surface for the deleted checker: Just recipes and help, package scripts, contributor commands, and CI steps.
- Remove the complete CI documentation job and every dependency reference to that job. Preserve all remaining jobs and their existing behavior.
- Remove documentation-check commands and requirements from root agent guidance, contributor guidance, current support documentation, and product milestone language.
- Retain the general whitespace diff check for documentation-only changes. This is not a replacement Markdown link or anchor checker.
- Remove the automatic documentation-link milestone criterion instead of substituting a new documentation-tooling promise.
- Update every tracked specification and ticket that names documentation checks, including both current feature trackers. Remove only that qualification requirement; retain behavioral, Rust, TypeScript, browser, synchronization, and other acceptance criteria.
- Preserve unrelated edits in mixed files, especially the active CI workflow rewrite. Use targeted edits and review the resulting dependency graph.
- Update the global setup skill to inspect existing agent configuration, ignore rules, contributor guidance, and current tracker contents during exploration.
- For local Markdown selection, require the tracker directory to be unignored and include that correction in the proposed setup when needed.
- Require the setup skill to identify generated artifacts already stored in the tracker directory and propose a separate temporary-artifact location. Include artifact moves and reference updates in the confirmation draft.
- Require the setup skill to remove contradictory tracker guidance and verify that tracker files are visible to Git after setup.
- Do not add a replacement Markdown linter, new validation framework, runtime dependency, product migration, or unrelated repository reconfiguration.
- Do not change other Just commands, Rustdoc generation, E2E type checking, product tests, or deployment behavior.

## Testing Decisions

- Use the existing repository command and configuration surfaces as the primary test seam. No new test-only interface is needed.
- Enumerate Just recipes and verify the command file parses after the deleted recipe and help entry are removed.
- Parse the package manifest and run the existing package-format check to verify that the deleted script leaves valid content and formatting.
- Inspect the CI workflow structure and confirm that no job or dependency references the removed documentation job. Use available YAML parsing or workflow validation tooling without adding a dependency for this purpose.
- Search tracked source, configuration, documentation, and tracker files for the deleted script name, command aliases, CI job name, and documentation-check acceptance language. The expected result is no remaining references.
- Confirm remaining validation commands are intact, including the whitespace diff check, E2E type checking, Rust checks, and product test commands.
- Run the general whitespace diff check on the final repository changes.
- Review the diff for mixed files and verify that unrelated working-tree changes are not staged or reverted.
- Check the global setup skill through its highest-level outcome using a representative setup scenario: an existing local tracker, a GitHub remote, an ignored tracker directory, conflicting guidance, and generated artifacts in the tracker directory. The proposed draft must select the existing tracker, make it visible to Git, separate temporary artifacts, and resolve conflicting guidance before writing.
- Verify the current repository tracker remains visible to Git and its temporary artifact directory remains ignored. Do not rerun the documentation checker being deleted.

## Out of Scope

- A replacement Markdown link, anchor, style, or prose checker.
- Removing or weakening unrelated product, Rust, TypeScript, browser, synchronization, OpenAPI, or deployment validation.
- Repeating the completed repository tracker setup or changing its selected tracker and triage vocabulary.
- Further work on retro items already declared fixed or expected.
- Absorbing, reverting, or committing unrelated working-tree and CI workflow changes.
- Product runtime changes, public API changes, migrations, or new dependencies.

## Further Notes

The repository portion of tracker setup was already completed and committed. This specification covers only the residual global setup-skill improvement and the complete removal of Markdown documentation checks.

The requested removal includes tracked specifications and tickets. Leaving acceptance criteria that reference the deleted facility would make those tickets internally inconsistent even after all executable entry points were removed.
