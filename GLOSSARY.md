# Shared domain

Canonical vocabulary for the Installation and its local-first data contexts. Ownership rules live in [target architecture](docs/architecture.md); protocol and implementation details live in the relevant topic documents.

## Language

**Installation**:
The top-level identity and policy boundary containing Users, Applications, Devices, permissions, and Resources.

**Node**:
A process that hosts Installation service contexts and retained local state.

**Sync Server**:
A Node exposed as an HTTPS synchronization backend. The term describes an operational role, not a privileged domain authority.

**User**:
A human or service identity registered with the IdP.

**Application**:
A logical group of OAuth Clients whose Users share a Resource namespace.

**OAuth Client**:
A concrete web, native, or machine integration for an Application.
_Avoid_: Application when referring to a concrete client registration

**Principal**:
The authenticated actor in a request: a User, OAuth Client, or service identity.

**Credential**:
A secret or public-key method for authenticating a User. Credentials are local to the IdP and are not Resource data.

**Token**:
A credential issued by the IdP for API identity and action constraints. A Token alone is not Device approval or Selection.

**Node Signer**:
An approved token-signing identity held independently by one Node.
_Avoid_: Replica Signer

**Role**:
A named collection of Permissions.

**Permission**:
An atomic action that Management can grant to a Principal for a target.

**Assignment**:
A grant of a Role to a Principal for a target.

**Device**:
An approved Installation endpoint that can retain authorized Resource replicas. A Device is not owned by one User and may serve multiple Users and Applications.

**Endpoint**:
A uniquely identified network participant. Endpoint identity alone does not establish Device approval or Resource access.

**Resource**:
A database or filesystem with stable identity in one `(User subject, Application)` namespace.

**Replica**:
A Device-local copy of a Resource.

**Selection**:
The desired replication relationship between one Resource and one Device.

**Residency**:
A Device-local choice about retained content: Full retains metadata and content; Passthrough retains metadata and reads content from an authorized Full provider.

**Tombstone**:
A synchronized deletion record that prevents an older replica from restoring deleted state.

## Database (`ofdb`)

**Database**:
The public control handle for storage lifecycle, hosting, and synchronization.

**Client**:
The public query handle for reads, writes, and explicit transactions.

**Logical Row**:
A SQL row with stable identity independent of its physical history.

**Generation**:
A retained identity for one KV value lifecycle, including deletion and later recreation.

## File System (`offs`)

**File System**:
Local-first replicated storage of paths, file metadata, and content.

**File Entry**:
Metadata for one path, including its stable file identity and content revision.

**Content Revision**:
The identity of one file metadata/content version.

## Endpoint Mesh (`ofnet`)

**Peer**:
An Endpoint known to another Endpoint as a possible mesh neighbor.

**Mesh**:
The peer relationships through which Endpoints communicate, including relayed communication.

**Broadcast**:
A message intended for every Endpoint in the Mesh.

**Direct Message**:
A message intended for one identified Endpoint, whether reached directly or through relays.

**Byte Stream**:
An ordered flow of bytes between two Endpoints without mesh message interpretation.
