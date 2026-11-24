# 05: Clarify User and endpoint identity in public contracts

Status: ready-for-agent

Work status: resolved

## User outcome

A client can tell which identifier selects a Device record, which key proves endpoint possession, and which subject identifies the User actor.

## Scope and compatibility

Clarify the existing contracts. Keep definitions in the context glossaries and ownership in the architecture. Put route and field descriptions in the OpenAPI source and regenerate affected contracts. Do not rename fields, alter the signing payload, move routes, or remove legacy owner records.

## Acceptance checks

- [x] The architecture and glossary distinguish IdP Principal identity and OAuth/OIDC trust from Device endpoint identity and approval. Management owns authorization, not Device configuration or records.
- [x] Device OpenAPI descriptions distinguish User subject, Device record ID, and endpoint public key. Where `ownerSubject` appears, state its legacy role without calling it the request actor or the endpoint identity.
- [x] Generated contracts match the source; schema shapes and wire behavior do not change.

## Dependencies and current evidence

None. The architecture and glossaries already define the split. Some Device descriptions still conflate approval or a trusted endpoint with IdP or legacy owner terminology. Check the public descriptions against the current handlers, not the shared-Device target.
