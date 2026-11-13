# Shared domain

Terms shared by IdP, Management, Device, and Storage. See [the glossary map](GLOSSARY-MAP.md) for context terms, design, and contracts.

## Language

**Installation**:
A group of equal Nodes sharing a canonical identity issuer and trusted identity, policy, and resource data.

**Node**:
An independently usable Installation member hosting IdP, Management, Device, and Storage, with its own local trusted state and private signer.
_Avoid_: Authority node, privileged replica, Storage-only Node

**Unified Host**:
The host composing IdP, Management, Device, and Storage while retaining separate domain ownership. Local development composition does not establish Installation membership or imply mesh participation.

**User**:
An authenticated human subject with a stable public identity.

**Application**:
A logical product or resource identified by a stable URI. It groups OAuth Clients and scopes storage resources owned by a User; it is not an OAuth Client.

**Principal**:
The User or OAuth Client represented by a token's subject. A Node Signer identifies the issuing Node, while Device endpoint identity authenticates transport; neither is the token's subject.

**Device**:
A Device-service-owned persistent domain identity bound to a transport endpoint ID, with a locally supplied name, public key, and reachable address. The Device service owns its User association, approval, transfer and revocation. An endpoint alone is neither an approved Device nor resource permission; a Device's transport identity is distinct from an IdP token-signing identity.

**Trusted Device**:
A Device-service-approved, non-revoked Device eligible to participate in the mesh. Approval is transport trust, not User authentication, token-signing authority or resource authorization.

**Bootstrap Service**:
The setup functionality that establishes a repeatable Installation baseline through separate IdP, Device, and Storage owner operations. It is not a separate setup owner or deployment broker.

**Installation Setup**:
The process that establishes a new Installation or joins it as an equal Node through a trusted peer.
_Avoid_: Master setup, primary-node setup

**Setup Owner**:
One of IdP, Device, or Storage, independently responsible for its admission capability, retained exact Installation Setup operation, readiness, and reset boundary. Management participates in the Installation but is not a setup owner.

**Setup Capability**:
Temporary authority issued by one Setup Owner and accepted only by that owner. It is not User authentication or authority over another owner.

**Device Setup**:
The process that configures and later edits the data an Installation member stores locally after Installation Setup completes.
_Avoid_: Setup Mode, device initialization

**Reset Device**:
The operation that removes one runtime's local setup state, synchronized data, and Device identity, returning it to Installation Setup. It does not remove data from other Devices.

**Initial Administrator**:
The username/password User whose supplied credentials establish administrative access and whose explicit initial permissions are granted when a new Installation is established.
_Avoid_: Default admin, bootstrap admin

**Trusted Peer**:
An approved Node whose authenticated identity and permitted shared data are trusted within an Installation; it has no privileged writer role and is not merely a known mesh Peer.

**Device Pairing**:
A purpose-specific exchange used to establish approved Installation Device membership. Pairing is not a general resource-access grant.

Endpoint, Peer, Mesh, Broadcast, Direct Message, and Byte Stream retain their [ofnet definitions](../ofnet/GLOSSARY.md); they do not imply Installation membership or resource authorization.
