# Glossary map

## Shared model

- [Shared vocabulary](GLOSSARY.md): Installation, Node, Sync Server, User, Application, Device, Resource, Replica, and Selection.
- [Product goal](docs/goal.md): north-star workflow, milestones, and non-goals.
- [Target architecture](docs/architecture.md): ownership and trust boundaries.
- [Current support](README.md#current-status): implemented and qualified behavior.

## Contexts

| Context    | Vocabulary                                             | Design and contracts                                                                                                                                                                                              |
| ---------- | ------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| IdP        | [Identity Provider](services/idp-service/GLOSSARY.md)  | [Design](services/idp-service/docs/design.md), [local security](services/idp-service/docs/local-security.md)                                                                                                      |
| Management | [Management](services/management-service/GLOSSARY.md)  | [Policy persistence](services/management-service/README.md), [target ownership](docs/architecture.md#context-ownership)                                                                                          |
| Device     | [Device](services/device-service/GLOSSARY.md)          | [Target ownership](docs/architecture.md#context-ownership)                                                                                                                                                        |
| Storage    | [Storage](services/storage-service/GLOSSARY.md)        | [Design](services/storage-service/docs/design.md), [API](services/storage-service/docs/api.md), [security](services/storage-service/docs/security.md), [sync](services/storage-service/docs/sync-architecture.md) |
| `ofdb`     | [Database terms](GLOSSARY.md#database-ofdb)            | [Overview](db/README.md), [design](db/design.md)                                                                                                                                                                  |
| `ofnet`    | [Endpoint Mesh terms](GLOSSARY.md#endpoint-mesh-ofnet) | [Overview](net/README.md), [crate](net/iroh-chain/README.md)                                                                                                                                                      |
| `offs`     | [File System terms](GLOSSARY.md#file-system-offs)      | [Overview](fs/README.md), [crate](fs/file-system/README.md)                                                                                                                                                       |

Each context covers its related model, service, server, and client packages. Definitions belong in the relevant glossary. Current contracts belong in stable topic files. Commit-specific evidence belongs in CI records, not in definitions or tracked task notes.

## Relationships

- IdP owns identity and OAuth trust. It does not own Devices or Resources.
- Management owns authorization policy. Device owns Device records and lifecycle.
- Device approval permits an Installation endpoint. Selection states which Resource should have a replica on that Device.
- Storage owns Resource catalogs and access. Resources remain in User/Application namespaces even when several Users share one Device.
- `ofdb` and `offs` own persistence and reconciliation. `ofnet` owns generic transport mechanics. `of` owns domain authorization and protocol adapters.
- Pairing is not general access. Allowlisting is not Resource authorization. Mesh messages are not transparent Resource synchronization.
