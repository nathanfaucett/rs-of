# 01: Remove Markdown documentation checks end to end

**What to build:** The unwanted Markdown link-and-anchor checker is removed as a complete facility. Its implementation, public commands, CI job, dependency edges, contributor instructions, milestone language, and tracker acceptance requirements disappear together. Remaining command manifests and CI dependencies stay valid, while unrelated product validation and working-tree changes remain intact.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

Work: resolved

- [ ] The Markdown checker implementation and every Just, package, contributor, and CI invocation are deleted without a replacement checker.
- [ ] The CI documentation job and every dependency on it are removed; all remaining jobs and dependency references stay valid.
- [ ] Documentation-check commands and automatic-link-check claims are removed from root guidance, contributor guidance, current support documentation, and product milestones.
- [ ] Every tracked specification and ticket removes documentation-check acceptance language while retaining behavioral and other qualification requirements.
- [ ] The general whitespace diff check and unrelated Rust, TypeScript, browser, synchronization, OpenAPI, and deployment validation remain available.
- [ ] Just command discovery parses correctly, and the package manifest passes its existing format validation.
- [ ] A complete tracked-file search finds no deleted script, command alias, CI documentation job, or documentation-check acceptance reference.
- [ ] The final diff preserves unrelated working-tree and CI workflow changes and passes the whitespace diff check.
