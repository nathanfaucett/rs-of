# Device

The Device lifecycle, endpoint-trust, and replication-policy context of an Installation. Shared terms are defined in [the root glossary](../../GLOSSARY.md).

## Language

**Device Service**:
The authority for Devices, endpoint-ID bindings, User ownership, approval/enrollment, transfer/revocation, restrictions, selections, and replication policy. It consumes IdP User identity and Management permissions. Endpoint possession alone grants neither membership nor resource access.

**Resource Selection**:
A whole-resource choice of what a Device retains and synchronizes, independent of ongoing User sessions. Its owner may select resources in their own subject/Application namespaces within administrator storage limits; deselection removes only the local copy, not the resource itself.
