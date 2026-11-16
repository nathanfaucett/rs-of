# 05: device-server / idp-server / setup — generic callback holders

**What to build:** Make the node-join source generic over its identity implementation (`Arc<I>` where `I` implements the join identity trait) so no `Arc<dyn NodeJoinIdentity>` remains; axum router state stays concrete per build site. The join-readiness, check-join-history, independent-registrar, and independent-readiness callbacks are stored as generic closure/future parameters on their holders (node-join source, join-history source, create lifecycle) instead of `Arc<dyn Fn(...) -> Pin<Box<dyn Future>>>`. The idp-server owner-provisioner join impls, the provisioning stage helper (returns the caller's future directly), and the owner-boundary integration tests are updated to match.

**Blocked by:** 04 (join traits and control-plane generics land first)

**Status:** ready-for-agent

- [ ] device-server, idp-server, and server-common setup crates compile with no boxed/dyn async callbacks
- [ ] Node-join and independent create/join tests pass, including owner-boundary interruption tests; clippy/rustfmt clean
