# 04: Make adding a Node understandable on the approving device

**What to build:** An authorized existing User can enable acceptance, inspect exact pending Node requests, and approve or reject a request within one understandable add-device task. This works with the existing Join request contract independently of the joining-device presentation change.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] Combine existing source acceptance controls and pending-request inspection into one coherent task, with clear instructions for the joining device.
- [ ] Show sufficient authenticated identity and offer information to recognize the intended Installation, Device, endpoint, owner, and independent signer bindings without presenting them as unnecessary configuration choices.
- [ ] Approve or reject only the exact selected pending request through existing authorized owner routes. Setup capabilities and tokens with the wrong audience cannot authorize source actions.
- [ ] Preserve current Management permissions, Device approval checks, independent IdP signer-enrollment checks, restrictions, and selections. Approval is not general resource authorization.
- [ ] Distinguish request acceptance from completion, including independently deployed owner behavior. Report partial enrollment/provisioning failure accurately and permit exact retry without replacement identities or automatic rollback.
- [ ] Rendered-flow tests exercise acceptance, pending offers, approval, rejection, unauthorized access, and actionable partial failure using existing owner-client adapters.
- [ ] Reuse the shared public Join lifecycle seam to verify source authorization and exact-offer behavior; do not build another approval runtime or test framework.
- [ ] Delete superseded approval navigation and duplicated coordination in the affected scope, update source approval documentation, and record focused validation outcomes.
