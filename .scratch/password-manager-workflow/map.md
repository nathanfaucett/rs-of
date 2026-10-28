# Password-manager workflow

## Notes

- The sole ticket contains the complete requirements; no separate spec was provided.
- Integration branch: `integration/password-manager-workflow`.
- Commits: `002131c8a` (workflow extraction and tests), `cb5373448` (teardown ordering and mounted warning assertion).

## Decisions-so-far

- Resolved [01: Make the secret editor consume one workflow interface](issues/01-simplify-secret-editor-workflow.md). One app-local reactive module owns lifecycle, settlement, recovery, and disposal; the view keeps draft bindings and rendering. Existing SDK, adapter, and official live-query boundaries remain unchanged.
- Workflow behavior is covered through its public interface with the real collection adapter and controlled transport; mounted tests focus on UI/authenticated-context integration rather than duplicating the workflow suite.
- Final validation: app 41, SDK 45, adapter 59, Storage client 21 tests passed; app type check and production build passed. See the ticket Answer for review and validation details.

## Fog

- No live Installation qualification was performed.
- `just crap` and the Rust feature-powerset suite timed out after 120 seconds during compilation; neither completed. No Rust source changed.
