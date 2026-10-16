# Management

The Device, endpoint-trust, resource-policy and role-based access-control context hosted by every Node of an Installation. Shared terms are defined in [the root glossary](../../GLOSSARY.md). Service ownership is described in [system design](../../docs/design.md); resource selection and admission are described in [Storage design](../storage-service/docs/design.md).

## Language

**Management Service**:
The authority for Devices, endpoint-ID bindings, User ownership, approval/enrollment, transfer/revocation, roles, permissions, assignments, restrictions, selections and replication policy. Users, credentials, Applications, OAuth Clients and token-signing identity/trust belong to IdP. Endpoint possession proves transport identity; current Management approval and policy determine permitted use.

**Permission**:
A named action with Application or Installation scope.

**Role**:
A group of explicit permissions whose assignments grant authority to a User.

**Application-scoped Permission**:
Authority for a named action within one Application; it grants no installation-wide authority.

**Installation-scoped Permission**:
Authority for a named action across an Installation, including administration of its infrastructure identities.

**Resource Selection**:
A whole-resource choice of what a Device retains and synchronizes, independent of ongoing user sessions. Its owner may select resources in their own subject/Application namespaces within administrator storage limits; deselection removes only the local copy, not the resource itself.
