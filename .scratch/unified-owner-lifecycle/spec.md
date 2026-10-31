# Converge Unified Host on the owner-local Installation Setup lifecycle

Status: ready-for-agent

## Problem Statement

The Unified Host is supposed to compose a Node's IdP, Management, Device, and Storage contexts while retaining separate domain and state ownership. Today it also carries a second, parallel Installation Setup implementation of its own: a single setup capability for the whole installation, host-side operation journals, a host metadata store, an endpoint credential file, a hosted setup worker, and its own Create and Join flows. The owner-local setup lifecycle used by standalone deployments — where IdP, Device, and Storage each admit, bind, publish, and retire their own exact operation behind their own protected capability — is implemented separately in the shared contract and in each owner.

Two consequences follow. Maintainers carry two setup lifecycles with different evidence formats, retry semantics, readiness rules, and reset boundaries, so a behavior change in one is routinely absent from the other. Operators of the Unified Host hold one capability that admits every context, whereas operators of the standalone deployment hold one capability per owner, so a single leaked Unified setup code authorizes the entire installation instead of one owner.

The public setup surface also misrepresents the design. Unified setup is nested under the IdP composition prefix even though setup coordinates IdP, Device, and Storage, and the owner-local surfaces carry an "independent" infix that describes a deployment topology rather than what they are. The same owner setup contract therefore has two different public shapes depending on which deployment serves it.

Finally, the two deployments are not qualified by one suite. The cross-layout deployment suite exercises both topologies with the same scenarios, but it is entangled with the root package's test layout, and the Unified Host crate carries roughly ten thousand lines of its own test files that repeat service-level and deployment-level behavior. The offline-first filesystem surface is qualified only in-process by the filesystem crate's own suite, which cannot reach either deployment.

## Solution

Use one Installation Setup lifecycle for both deployments: the owner-local one already implemented in the shared contract and in each owner.

The Unified Host hosts the IdP, Device, and Storage owner setup workers inside its own process, exactly as the standalone deployment hosts them in their own processes. Each owner keeps its own setup capability, its own admission state, its own retained exact operation, its own published readiness, and its own reset boundary. The Unified Host keeps no operation journal of its own; the owners are the record.

Every owner keeps one setup surface, at its composition prefix, in both deployments. A standalone owner serves it at the root of its own origin; the Unified Host serves the same surfaces at the IdP, Device, and Storage prefixes of its single composed endpoint. There is no separate deployment-level setup endpoint and no broker between the operator and the owners: the operator supplies the base APIs of the four contexts and the secret codes once, and the client presents each owner's capability to that owner and no other, so a request never carries more than one capability and a capability never travels to an owner it does not belong to.

Management is not a setup owner in either topology. Its participation is driven by configuration the operator supplies once — the base APIs of the four contexts and the secret codes — so there is no extra Management ceremony and no separate operator action.

Trust stays local and singular. The only certificate authority involved is the one for the local dynamic setup server that the desktop application talks to. The owners hosted inside the Unified Host sit behind that server and need no additional authority of their own; the command line pins the local authority by path, and the desktop application installs the single authority it uses.

The Unified Host crate is reduced to composition, configuration, command line, installation endpoint identity, and the owner setup surfaces it hosts. The parallel host lifecycle, its journals, its metadata store, its endpoint credential file, and the test files that repeat service or deployment behavior are deleted rather than wrapped.

One cross-layout deployment suite qualifies both topologies with the same scenarios and gains the offline-first filesystem coverage that only exists in-process today.

## User Stories

1. As a Node operator, I want the Unified Host and the standalone deployment to use the same Installation Setup lifecycle, so that a behavior I rely on is identical in both.
2. As a Node operator, I want one setup capability per owner in every topology, so that a leaked capability authorizes one owner rather than the whole installation.
3. As a Node operator, I want the IdP owner's capability to be useless against the Device and Storage owners, so that compromising one owner's setup does not compromise the others.
4. As a Node operator, I want each owner to bind, publish, and retire its own exact operation, so that no deployment holds a second copy of setup progress that can disagree with the owners.
5. As a Node operator, I want the Unified Host to keep no setup journal of its own, so that there is one authoritative record of an in-flight operation.
6. As a Node operator, I want every owner to keep its own setup surface in both topologies, so that I learn one procedure regardless of how the deployment is arranged.
7. As a Node operator, I want a standalone owner to serve setup at the root of its own origin and a composed owner to serve setup at its composition prefix, so that the same contract addresses the same owner either way.
8. As a Node operator, I want the owner setup surface to be named for what it is rather than for the deployment topology that uses it, so that the public contract does not change when the topology changes.
9. As a Node operator, I want the Unified Host to serve the IdP, Device, and Storage setup surfaces on its single composed endpoint, so that setup is not presented as an IdP-only operation.
10. As a Node operator, I want no request to carry more than one owner's capability, so that a captured request cannot authorize an owner it was not issued for.
11. As a Node operator, I want a capability to be accepted only by the owner that issued it, so that a request cannot be replayed against a different owner.
12. As an Initial Administrator, I want to enter the base APIs and secret codes once, so that installing a Node is one operator interaction.
13. As an Initial Administrator, I want Management to participate from the configuration I supplied, so that I am not asked for a separate Management registration step.
14. As an Initial Administrator, I want the client to present each owner's capability only to that owner, so that no owner receives another owner's secrets.
15. As an Initial Administrator, I want the administrator password delivered only to the IdP owner, so that the Device and Storage owners never hold it.
16. As an Initial Administrator, I want the same exact operation presented to every owner, so that the owners agree on one installation rather than three partial ones.
17. As an Initial Administrator, I want an exact retry to be accepted after an interruption, so that a failed attempt does not create a second installation.
18. As an Initial Administrator, I want a conflicting operation rejected, so that a concurrent caller cannot change an in-flight setup.
19. As an Initial Administrator, I want the operator session established by the setup result, so that I can use the Installation immediately after it completes.
20. As a joining Node operator, I want the same owner-local Join behavior in both topologies, so that joining a peer does not depend on the deployment shape.
21. As a joining Node operator, I want the Trusted Peer and authenticated issuer retained with the operation, so that a restart cannot substitute another installation.
22. As a joining Node operator, I want one owner-local capability per owner during Join, so that the join authority of my Node is scoped the same way as its create authority.
23. As a Node operator, I want the Unified Host to publish its installation endpoint through the owner-local peer facts, so that owners learn the endpoint the same way in both topologies.
24. As a Node operator, I want the canonical issuer to be the IdP base in both topologies, so that tokens and audiences do not depend on the deployment shape.
25. As a Node operator, I want the only certificate authority to be the one for the local dynamic setup server, so that I do not have to trust an authority per owner to install my own Node.
26. As a desktop application user, I want to install one local authority rather than one per owner, so that installing the application is simpler.
27. As a desktop application user, I want the application to talk to each owner's setup surface, so that it does not depend on an IdP-prefixed setup path.
28. As a command line user, I want to pin the local setup authority by path rather than install it, so that setup never mutates operating system trust.
29. As a command line user, I want the same setup commands to work in both topologies, so that I do not learn two procedures.
30. As a standalone deployment operator, I want each owner to keep its own setup surface and capability, so that the multi-process deployment is not changed to satisfy the unified one.
31. As a standalone deployment operator, I want the standalone owner surfaces to remain usable without a unified deployment present, so that the shared contract stays layout-independent.
32. As a Node operator, I want each owner to be resettable without affecting the others, so that one damaged owner does not require reinstalling the Node.
33. As a Node operator, I want an unsupported prior local state refused with an actionable message, so that an upgrade never silently replaces retained setup evidence.
34. As a Node operator, I want completed Create retries to keep their existing checks, so that a retired capability is never accepted as login authority.
35. As a Node operator, I want cancellation to stop eligible work without claiming rollback, so that retained owner changes are not mistaken for a fresh host.
36. As a Node operator, I want restart after a completed setup to use existing local state without the original Join peer, so that my Node remains usable offline.
37. As a filesystem user, I want a filesystem resource created through the deployment, so that the offline-first filesystem is qualified against a real deployment.
38. As a filesystem user, I want file content written and read through the deployment in both topologies, so that the deployment-level file behavior is not assumed from in-process tests.
39. As a filesystem user, I want residency and deletion verified through the deployment in both topologies, so that device-local residency and tombstone behavior survive composition.
40. As a filesystem user, I want the filesystem resource readable after a retained restart in both topologies, so that composed state survives restart.
41. As a maintainer, I want the Unified Host crate to contain only composition, configuration, command line, endpoint identity, and hosted owner setup surfaces, so that its size reflects its responsibility.
42. As a maintainer, I want the parallel host lifecycle deleted rather than wrapped, so that there is one implementation of Installation Setup to reason about.
43. As a maintainer, I want the duplicated test files deleted after their coverage is confirmed elsewhere, so that test volume reflects the behavior under test.
44. As a maintainer, I want one cross-layout deployment suite to qualify both topologies, so that a divergence between topologies is caught by the suite.
45. As a maintainer, I want the filesystem crate's in-process suite kept as-is, so that internal synchronization and residency behavior keeps its fast coverage.
46. As a maintainer, I want the offline-first filesystem deployment scenarios written once and run against both topologies, so that the two deployments cannot drift in filesystem behavior.
47. As a maintainer, I want no new abstraction with a single implementation, so that the convergence does not add a layer to maintain.
48. As a maintainer, I want no deployment-level setup broker or aggregated readiness endpoint, so that the setup contract stays the owner-local one in both topologies.
49. As a maintainer, I want the generated clients and published API descriptions updated for the renamed setup surface, so that consumers are not left calling a path that no longer exists.

## Implementation Decisions

- Converge on the owner-local setup contract that already exists in the shared API crate and is implemented by the IdP, Device, and Storage owners. Do not add a third setup variant and do not keep a compatibility shim for the current Unified Host lifecycle.
- The Unified Host composes three owner setup workers in one process, one for each setup owner, using the same owner-local entry points the standalone binaries use. The difference between topologies is process boundaries and which context each process hosts, not the lifecycle.
- Setup owners are IdP, Device, and Storage in both topologies. Management is not a setup owner in either topology.
- Each owner retains its own setup capability, admission state, exact retained operation, published readiness, and reset boundary. A capability authorizes exactly one owner.
- The owner setup surface is the only setup surface. A standalone owner serves it at the root of its own origin; the Unified Host serves the IdP, Device, and Storage setup surfaces at the corresponding prefixes of its single composed endpoint.
- The current "independent" infix is removed from the owner setup path, because it names a deployment topology rather than the surface.
- There is no deployment-level setup broker, no unified setup endpoint, and no aggregated readiness document. The operator or client presents one owner's operation and that owner's capability per request, directly to that owner.
- The client collects the operator's input once — the base APIs of the four contexts and the secret codes — and then performs one owner-scoped request per setup owner. No request carries more than one capability.
- The administrator password travels only to the IdP owner. Management's client credentials travel only in the IdP owner's operation, where Management's client is registered through the generated IdP operations.
- The same public part of the operation is presented to every owner, so the owners agree on one installation rather than three partial ones.
- Management's participation follows from the supplied configuration and is not a separate step and not a separate operator action.
- The canonical issuer is the IdP base in both topologies. The Unified Host keeps the issuer it already derives from its IdP base and must not introduce a second logical installation issuer.
- The Unified Host publishes one installation endpoint, and publishes it through the owner-local peer facts that the owners already consume. Owners must not learn the endpoint through a unified-only channel.
- Trust model: the only certificate authority is the one for the local dynamic setup server. The owners hosted inside the Unified Host sit behind that server and require no additional authority. The command line pins the local authority by path; the desktop application installs the single authority it uses. Standalone owners keep their own local setup authority, pinned by path by the command line.
- Delete the Unified Host's parallel lifecycle: its Create and Join operation journals, its host metadata store, its hosted setup worker and setup host, its bootstrap configuration, credential bootstrap, bootstrap verification, and endpoint credential file. Nothing replaces them with a wrapper, mirror, or migration path.
- Retain the Unified Host's composition responsibility: composing the four owner routers behind one endpoint with the documented prefixes, the configuration that selects them, the command line, and installation endpoint identity.
- The Unified Host's test surface reduces to one composition test covering the composed path map, prefix composition, single endpoint, absence of double prefixing, and the setup surface paths. Delete the Unified Host's test files that repeat owner-service behavior, owner-contract behavior, or deployment-suite behavior, after confirming each behavior is covered at its owning seam.
- The replacement is atomic: an owner worker and the previous hosted path share the same owner state, so a host must never be half-provisioned by both. Replace Create and Join together and delete the previous implementation in the same change. Do not ship an expand–contract overlap.
- Do not introduce a new package, trait with a single implementation, adapter layer, facade, or deployment-level setup contract for the convergence. The existing shared contract, owner entry points, and owner setup surfaces are sufficient.
- The offline-first filesystem crate keeps its own in-process test suite unchanged; that suite is the coverage for synchronization, residency, admission, and retirement internals that a deployment cannot reach.
- Deployment-level filesystem qualification lives with the cross-layout deployment suite, which launches the production entry points of both topologies and runs each scenario once per topology.
- Preserve the existing observable differences between the topologies that are already documented — for example the retained-offer rejection status codes — rather than flattening them into a shared fiction.
- Detect the previous Unified Host local layout and refuse it with an actionable message; do not migrate, dual-read, or dual-write it.
- Update the generated clients and published API descriptions for the renamed owner setup surface, and update the desktop application, operator documentation, configuration samples, and the deployment descriptions that name the old paths.

## Testing Decisions

- The primary seam is the existing cross-layout deployment suite that launches the production service entry points, one child process per context, and drives them through generated clients. This is the highest available seam and it is where both topologies are already parameterized by a single scenario body.
- Test only external behavior: which paths exist, which capability is accepted, what the operation and status documents contain, what a retry does, what restart preserves, and what is retired. Do not assert on journal formats, table names, file layouts, or internal state representations.
- Extend the shared deployment scenario so the same body runs against the unified topology and the multi-process topology: fresh Create, exact retry, conflicting retry, each owner's readiness, Management participation from supplied configuration, retained restart, and Join.
- Add the offline-first filesystem behaviors to that shared scenario so both topologies are qualified identically: filesystem resource creation and read, file content write and read through the storage protocol, residency change and its effect, resource deletion and tombstone behavior, and reading the resource after a retained restart.
- Keep the owner-local contract tests at the contract seam: admission acceptance and rejection, capability retirement, challenge-bound cross-owner client registration, exact operation canonicalization, and readiness publication.
- Keep exactly one composition test in the Unified Host crate: the composed path map, prefix composition, single endpoint, absence of double prefixing, and the setup surface paths. This is the only behavior that belongs to a composition host.
- Prior art: the existing unified and independent named deployment scenarios, the existing composition test, the existing owner-local setup contract tests, and the filesystem crate's in-process suite.
- Use isolated temporary roots, the mock credential provider, mock operating system trust commands, explicit loopback addresses, bounded deadlines, and serial execution for the deployment suite; keep it opt-in and process-launching as it is today.
- The filesystem crate's in-process suite remains the fast inner loop and must still pass unchanged.
- Validation is the affected suites plus the workspace feature-powerset run in both workspaces. Record the exact commands run and their results; do not claim validation that was not run.

## Out of Scope

- A deployment-level setup broker, a unified setup endpoint, or an aggregated readiness document.
- Changing IdP, Management, Device, or Storage domain semantics, permissions, resource lifecycles, or replication behavior.
- Changing the offline-first filesystem crate's synchronization, residency, admission, or retirement algorithms.
- Migrating an existing Unified Host installation to the new setup surface; there is no legacy migration, dual read, or dual write.
- Changing the native peer protocol, endpoint mesh, pairing, or Trusted Peer admission.
- Moving the cross-layout deployment suite into the filesystem crate's workspace, or adding a cross-workspace dependency cycle to host it there.
- The separate host-state consolidation work, which owns the Unified Host's metadata storage and is unaffected by this convergence except where it referenced the deleted lifecycle.
- Changing the storage, management, or device public APIs beyond the setup surface rename.

## Further Notes

- The shared glossary describes the Bootstrap Service as establishing the baseline "through separate IdP and Device owner operations". This design uses three setup owners — IdP, Device, and Storage — so that description needs updating to match the contract as implemented.
- The setup surface rename changes generated client operation groups and published API descriptions, so client generation and snapshots are part of the change rather than a follow-up.
- The desktop application currently drives the Unified Host's setup and reset flow, so it carries the largest migration surface of any consumer.
- The Unified Host's trust footprint shrinks rather than grows: one local setup authority instead of one per owner.
- Deletion is the bulk of the work. The convergence is finished when the parallel lifecycle, its journals, its metadata store, and the repeated test files are gone and both topologies are qualified by one suite.
