# Storage glossary

**Storage Resource**:
A database or filesystem with a stable, kind-specific identity in one `(User subject, Application)` namespace.

**Resource Namespace**:
The `(User subject, Application)` boundary that contains a Resource. OAuth Clients of the same Application share a User's namespace; other subjects and Applications do not.

**Resource Ownership**:
Membership of a Resource in one Resource Namespace. It never means ownership by a Device.

**Resource Catalog**:
The record of Resource identity, namespace, display name, and deletion state. Catalog discovery does not imply local content possession or access permission.

**Storage-audience Access Token**:
An IdP access token whose audience and actions authorize Storage API requests. It is not a Device approval, Resource Selection, or mesh-transport identity.

**Filesystem ID**:
The stable identity of a filesystem Resource, independent of any local root path.

**Database ID**:
The stable identity of a database Resource, distinct from every Filesystem ID and control-plane record.

**Residency**:
Device-local policy for filesystem content. Full retains local content; Passthrough retains metadata and requires an authorized Full peer for reads. Residency is not Resource Selection or synchronized ownership.

**File Entry**:
Metadata for one filesystem path, including its stable identity, content revision, and provider information.

**Tombstone**:
A synchronized deletion record that prevents an older File Entry or Resource state from returning after reconciliation.
