# Ticket 08: Fix Docker build context and container contract test

Status: ready-for-agent

Blocked by: 01-copy-remaining-ofdb-crates, 02-rewrite-cargo-path-refs

## What it delivers

Docker image builds from `of/` alone; `scripts/test_container.py` passes with the single-repo layout.

## Scope

1. **`of/Dockerfile`**: remove `COPY ofdb/ /workspace/ofdb/`, `COPY offs/ /workspace/offs/`, `COPY ofnet/ /workspace/ofnet/` lines; build context is now `of/` only
2. **`of/Dockerfile.dockerignore`**: remove the allowlist entries `!ofdb/`, `!ofdb/**`, `!offs/`, `!offs/**`, `!ofnet/`, `!ofnet/**`
3. **`of/scripts/test_container.py`**: update the two tests:
   - `test_image_launches_full_node_cli_with_overridable_commands`: assert `COPY of/ /workspace/of/` (single copy) instead of four COPY lines; `WORKDIR /workspace/of`
   - `test_parent_context_is_allowlisted_and_prunes_nested_private_state`: assert dockerignore allowlist only has `!of/`, `!of/**` (drop ofdb/offs/ofnet); assert private state exclusions still present
   - `test_manifest_path_dependencies_fit_the_four_repository_context`: change `repositories = {"of", "ofdb", "offs", "ofnet"}` to `{"of"}` and resolve paths under `of/`

## Acceptance

- `docker build -f Dockerfile .` succeeds from `of/` parent directory
- `python3 scripts/test_container.py` passes all tests
- Built image runs `unified-server --help`
