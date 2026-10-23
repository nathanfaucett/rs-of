# Status: resolved

## Problem Statement

The `unified-server` crate is currently a "fat" wrapper rather than a thin one. While it correctly composes the IdP, Management, and Storage service routers, it also carries the entire implementation of the "Independent" (local) path for node bootstrapping, provisioning, and joining. This includes logic for local credential management, identity discovery, and node-joining state machines that are currently implemented within `unified-server` itself. This violates the design principle that `unified-server` should be a minimal, purpose-built host that coordinates existing service contexts without introducing new domain-specific logic or heavy dependencies.

## Solution

Refactor `unified-server` into a truly thin wrapper by extracting all "Independent/Local" functionality and relocating it to the appropriate service crates (primarily `idp-server`). The `unified-server` will be reduced to pure router composition, runtime assembly, and necessary protocol/adapter wiring.

## User Stories

1. As a system architect, I want `unified-server` to be a minimal composition layer, so that the boundary between the host and the services is clearly defined.
2. As a developer, I want to find all node-provisioning and joining logic within `idp-server`, so that I don't have to search multiple crates for the same domain logic.
3. As a maintainer, I want `unified-server` to have a minimal file count, so that the project structure is easy to navigate.
4. As a developer, I want the `unified-server` CLI to interact with the services (or a shared library) rather than implementing the business logic itself.
5. As a tester, I want to verify that the `unified-server` can still perform bootstrapping and joining, even though the implementation has moved.
6. As an operator, I want to ensure that the "Hosted" API routes are still correctly exposed via the `unified-server` composer.

## Implementation Decisions

- **Logic Relocation**: Move the following modules and their associated logic from `unified-server` to `idp-server` (or relevant service crates):
  - `local_join.rs` (and its logic) $\rightarrow$ `idp-server`
  - `local_provisioning.rs` $\rightarrow$ `idp-server`
  - `bootstrap_config.rs`, `bootstrap_verification.rs`, `credential_bootstrap.rs` $\rightarrow$ `idp-server`
  - `issuer_discovery.rs` $\rightarrow$ `idp-server`
  - `local_tls.rs` $\rightarrow$ `idp-server`
- **Module Cleanup**: Remove all `independent_...` test modules and associated logic from `unified-server`.
- **CLI Refactor**: Update `unified-server/src/cli.rs` to use the relocated logic via the service crates (or a shared internal library), ensuring the CLI remains functional.
- **Adapter Preservation**: Keep `setup_runtime.rs` and `security_sync.rs` in `unified-server` as they serve as wrapper-level coordination/adapter logic for the hosted environment.
- **Dependency Management**: Ensure that moving logic to `idp-server` does not create circular dependencies between `idp-server` and `unified-server`.

## Testing Decisions

- **Composition Testing**: Maintain and run existing `router.rs` tests to ensure the composed router still correctly handles the three service prefixes.
- **Integration Testing**: Run `tests/composition.rs` (and similar) to verify that the hosted routes (after the move) are still accessible via the `-unified-server` endpoint.
- **Functionality Verification**: Ensure that the `unified-server` CLI still performs its tasks (create, join, etc.) successfully after the underlying logic has been moved.

## Out of Scope

- Changing the actual logic of how a node joins or provisions.
- Moving the `security_sync` or `setup_runtime` logic (these are considered wrapper-level adapters).
- Adding any new features to the services.

## Further Notes

- The distinction between "Independent" (local) and "Hosted" (service-backed) paths is the primary driver for this refactor.
- The `idp-server` should become the source of truth for identity and provisioning logic.
