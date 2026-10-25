# 04: Make adding a Node understandable on the approving device

**What to build:** An authorized existing User can enable acceptance, inspect exact pending Node requests, and approve or reject a request within one understandable add-device task. This works with the existing Join request contract independently of the joining-device presentation change.

**Blocked by:** None (can start immediately).

Status: resolved

- [x] Combine existing source acceptance controls and pending-request inspection into one coherent task, with clear instructions for the joining device.
- [x] Show sufficient authenticated identity and offer information to recognize the intended Installation, Device, endpoint, owner, and independent signer bindings without presenting them as unnecessary configuration choices.
- [x] Approve or reject only the exact selected pending request through existing authorized owner routes. Setup capabilities and tokens with the wrong audience cannot authorize source actions.
- [x] Preserve current Management permissions, Device approval checks, independent IdP signer-enrollment checks, restrictions, and selections. Approval is not general resource authorization.
- [x] Distinguish request acceptance from completion, including independently deployed owner behavior. Report partial enrollment/provisioning failure accurately and permit exact retry without replacement identities or automatic rollback.
- [x] Rendered-flow tests exercise acceptance, pending offers, approval, rejection, unauthorized access, and actionable partial failure using existing owner-client adapters.
- [x] Reuse the shared public Join lifecycle seam to verify source authorization and exact-offer behavior; do not build another approval runtime or test framework.
- [x] Delete superseded approval navigation and duplicated coordination in the affected scope, update source approval documentation, and record focused validation outcomes.

## Answer

Implemented the source-only **Devices → Add a device** task in `SourceApproval.svelte` and wired it into the existing Devices page. Acceptance, trusted source endpoint/QR, independent-owner peer instructions, pending identity/owner/signer details and exact approve/reject actions now live together; removed the superseded page-local approval coordination and separate endpoint section.

The existing owner adapters and authorization contracts are unchanged: ordinary Management User bearer, exact offer IDs, Device approval and independent IdP signer checks remain authoritative. No setup capability, policy widening, automatic identity replacement or rollback was introduced. Source 202/200 responses explicitly leave target provisioning/current owner readiness unproven. Partial approval errors survive refresh and retain the exact offer for explicit retry; rejection failures direct the user to refresh the same pending request.

Validation: test-first missing-component failure and a behavioral red showing refresh erased partial failure, then green. Six new rendered cases pass through real owner-client adapters, plus three existing rendered Residency cases (9 total) and all 27 existing owner-client tests. `pnpm check`: zero errors/warnings. Both changed Svelte files: autofixer reports no issues/suggestions. Updated `docs/operations.md`.

Actual command summaries, prerequisites, TDD evidence and limitations: [source approval evidence](../evidence/04-source-approval.md). The existing shared public HTTPS Join lifecycle test was invoked rather than adding another runtime/fixture, but timed out after 120 seconds during dependency compilation; runtime source authorization/identity qualification is not claimed. Required Cargo Hack and CRAP runs also timed out compiling. The broad auth frontend suite had 24 failures/177 passes; the focused owner suite passes independently. Native GUI/OS-provider/WAN qualification was not performed. These validation limits do not change the implementation's source-only scope.
