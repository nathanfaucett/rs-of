# Management

The role-based access-control context of an Installation. Shared terms are defined in [the root glossary](../../GLOSSARY.md).

## Language

**Management Service**:
The authority for roles, permissions, and User assignments. It authorizes actions in the IdP, Device, and Storage contexts, but does not own their records or lifecycle.

**Permission**:
A named action with Application or Installation scope.

**Role**:
A group of explicit permissions whose assignments grant authority to a User.

**Application-scoped Permission**:
Authority for a named action within one Application; it grants no installation-wide authority.

**Installation-scoped Permission**:
Authority for a named action across an Installation, including administration of its infrastructure identities.
