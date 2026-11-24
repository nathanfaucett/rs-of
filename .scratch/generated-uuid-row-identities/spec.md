# Generate UUIDv7 Logical Row identities internally

Status: ready-for-agent
Work: resolved
Milestone: M2 — Single-Node encrypted synchronization
Dependencies: None identified.

## Problem Statement

An application or service caller can currently supply a UUIDv5 or UUIDv7 as a single-UUID primary key. The Automerge-backed SQL engine uses that value directly as the Logical Row identity and document identity. Reconstruction and generic document update paths can also derive an Automerge actor from that identity.

This bypasses the engine's UUIDv7 generation and allows caller-selected values to control storage and scan ordering. It also prevents the engine from guaranteeing that new Logical Row identities follow one generation policy.

The desired contract is that local callers cannot choose these identities. The engine must generate every new Logical Row identity as UUIDv7. Backward compatibility, migration support, and new storage or protocol versions are explicitly unnecessary.

## Solution

Reject every caller-supplied value for a single-UUID primary key during local insertion into the Automerge-backed SQL Database. Continue to generate UUIDv7 when that primary key is omitted or set to DEFAULT. Apply the rule uniformly to SQL text, bound parameters, typed statements, multi-row inserts, and explicit transactions, whether the Client is embedded or connected to an ofdb remote server.

Preserve UUID values in non-primary-key columns, predicates, references, and results. Preserve existing behavior for natural and composite primary keys, which already use a separate generated Logical Row identity.

Update affected in-repo services and schemas so they use the new contract. Services that do not need predetermined identities omit the primary key and consume the generated value returned by the insert. Services that need stable domain identities decouple those identities from the single-UUID primary-key row identity, using a natural or composite primary key and a separate unique domain identifier where required.

Replication between updated Databases continues to transfer peer-generated UUIDv7 Logical Row identities. The local insertion rule must not reject those replicated records. Legacy UUIDv5 Logical Row data may be rejected. Existing stores and histories require fresh initialization; no migration or compatibility path will be added.

## User Stories

1. As an application developer, I want the Database to generate a UUIDv7 primary key when I omit it, so that every new Logical Row receives an engine-owned identity.
2. As an application developer, I want DEFAULT to produce the same engine-generated UUIDv7 behavior, so that explicit default handling does not bypass the contract.
3. As an application developer, I want generated primary keys available through RETURNING, so that I can use them in later statements and domain operations.
4. As an application developer, I want an explicit UUID primary key in SQL text to be rejected, so that I cannot accidentally control Logical Row identity or ordering.
5. As an application developer, I want explicit UUID primary keys supplied through bound parameters to be rejected, so that parameterization does not bypass the rule.
6. As a service developer, I want explicit UUID primary keys in typed statements to be rejected, so that typed and textual APIs have the same contract.
7. As a service developer, I want the rule applied inside explicit transactions, so that transaction use does not create an alternate insertion path.
8. As a remote Client user, I want the server-side engine to enforce the same rule, so that remote execution cannot bypass local insertion policy.
9. As an application developer, I want a rejected multi-row insert to leave no inserted rows, so that one caller-supplied identity cannot produce partial writes.
10. As an application developer, I want an explicit primary key rejected before ON CONFLICT handling, so that conflict branches cannot preserve or update a caller-selected Logical Row identity.
11. As an application developer, I want UUID values in ordinary columns and predicates to remain valid, so that the restriction does not remove UUID as a domain data type.
12. As a service developer, I want natural and composite primary keys to retain their existing behavior, so that stable domain identity can be represented without choosing the internal Logical Row identity.
13. As a service developer, I want deterministic domain identifiers separated from generated row identities, so that resource lookup and Selection semantics can remain stable without caller-controlled Automerge document identities.
14. As a Node operator, I want updated Replicas to synchronize newly generated UUIDv7 rows, so that local generation rules do not prevent convergence.
15. As a Node operator, I want legacy UUIDv5 Logical Row histories to be unsupported, so that the implementation does not need a hidden compatibility path.
16. As a database maintainer, I want one existing UUIDv7 generation path reused for user rows and catalog rows, so that identity generation does not acquire another implementation.
17. As a database maintainer, I want generated and supplied values distinguished before insert materialization erases their origin, so that generated IDs are not rejected by the new guard.
18. As a database maintainer, I want new SQL document identities to be UUIDv7 before any actor derivation occurs, so that reconstruction cannot reintroduce caller-selected actor identities through supported local inserts.
19. As a browser application developer, I want the WASM Client to expose the same rejection and generation behavior, so that browser storage follows the shared SQL contract.
20. As a maintainer, I want current documentation and fresh-initialization schemas updated without migration guidance, so that users do not infer unsupported compatibility guarantees.

## Implementation Decisions

- Own the policy in the SQL engine's local insert execution path. Do not implement it in SQL translation, the remote protocol, TypeScript wrappers, or individual services.
- Preserve the caller-supplied-versus-generated distinction until the admission check completes. A check after row materialization cannot distinguish an explicit UUIDv7 from one generated while filling the primary key.
- Cover both typed insertion forms: whole-row inserts and column/value inserts. SQL text and parameters already converge on these forms.
- Reject an explicit single-UUID primary-key value before duplicate checks, tombstone checks, conflict handling, or persistence. Return the existing invalid-query error category with a clear generated-only message.
- Continue generating omitted and DEFAULT primary keys through the engine timestamp provider and the existing UUIDv7 helper. Do not add a generator dependency or a second identity-generation interface.
- Keep primary keys immutable. Do not add an update or conflict-resolution path that changes a Logical Row identity.
- Keep UUID columns that are not the single-UUID primary key unchanged. Natural and composite primary keys remain valid and continue to receive a separate generated internal identity.
- Require UUIDv7 for SQL Logical Row identities at engine boundaries. UUIDv5 support for these identities is removed rather than retained for old records.
- Keep replication application separate from local insertion admission. A Replica must accept a valid peer UUIDv7 identity during snapshot, state, or native-change application. The protocol cannot prove whether a peer originally obtained that UUID through its local Client, so the guarantee concerns supported local creation paths rather than cryptographic provenance.
- Preserve the current Automerge document and actor derivation design. New supported SQL rows supply UUIDv7 document identities; this specification does not introduce a replacement actor model.
- Update affected Device, IdP, Management, and Storage persistence callers to the generated-only contract. Use generated RETURNING values where callers currently generate UUIDv7 immediately before insertion.
- Where current code relies on deterministic UUIDv5 primary keys, replace that schema identity with a natural or composite primary key and, when needed, a separate unique domain identifier. Do not bypass the new guard by moving caller-selected UUIDs into another single-UUID primary key.
- Update fresh-initialization schemas and migration fixtures in place. Do not add an upgrade migration, migration version, storage format version, synchronization protocol version, or legacy wire adapter. Old stores may be rejected and recreated.
- Remove public documentation that promises explicit UUIDv5 or UUIDv7 primary-key insertion. Document generated-only local insertion, unchanged replication of peer-generated identities, and the fresh-store requirement.
- Keep the remote statement and value wire shapes unchanged. UUID values remain necessary for ordinary columns, predicates, references, and replicated state.
- The PostgreSQL-only query backend is unchanged because it does not create Automerge Logical Rows or actor identities. KV is also unchanged because it already generates UUIDv7 Generations internally.

## Testing Decisions

- Use the existing public SQL Client query and transaction API as the main test seam. Test external behavior through the same calls available to applications; do not add a test-only insertion interface.
- Exercise SQL CAST literals, bound UUID parameters, typed whole-row inserts, and typed column/value inserts. Prove explicit UUIDv5 and UUIDv7 primary keys are rejected through each applicable form.
- Prove omitted and DEFAULT primary keys succeed, return distinct RFC-variant UUIDv7 values, and persist those returned identities.
- Prove explicit values are rejected in multi-row batches without partial writes and in ON CONFLICT statements before either conflict branch executes.
- Prove explicit NULL retains its existing primary-key validation behavior rather than being treated as DEFAULT.
- Prove UUID values in non-primary-key columns, foreign references, predicates, and ordinary results remain accepted.
- Prove natural and composite primary keys retain their existing insertion and generated-internal-identity behavior.
- Run the same public Client contract through embedded and remote execution. Reuse the existing embedded client, transaction, remote execution, and rollback test patterns.
- Test the public Database synchronization seam between fresh updated peers. Insert through one Client, synchronize, and prove the peer retains the generated UUIDv7 identity and converges without treating replication as a forbidden local insert.
- Extend existing Automerge reconstruction and history tests to establish that newly supported SQL document identities and any identities derived from them are UUIDv7. Test behavior rather than private actor fields where a public history observation can establish the contract.
- Update affected service repository and public service API tests. Cover generated IDs, deterministic Selection and Resource lookup after schema decoupling, conflict behavior, restart with fresh stores, and synchronization between updated owners.
- Extend the existing real-backend browser SQL tests for omitted-key generation and explicit-key rejection. Run filtered package build, test, lint, and type checks, then affected dependents; unit tests alone are insufficient for the browser behavior change.
- Use the existing schema identity tests, SQL client and remote tests, Automerge history and convergence tests, service repository tests, and browser binding tests as prior art.
- Run focused Rust tests first, then formatting, focused Clippy, the appropriate feature-powerset checks, and CRAP checks for the changed Rust surface. Record any unrun broader qualification explicitly.

## Out of Scope

- Backward compatibility for UUIDv5 Logical Row data or caller-supplied UUID primary keys.
- Data migration, upgrade migrations, new migration versions, storage format versions, synchronization protocol versions, or legacy adapters.
- A guarantee of strict wall-clock ordering, insertion ordering within one timestamp, or ordering across Nodes with clock skew.
- A new Automerge actor model or a redesign of generic B-tree document IDs and actor derivation.
- Changes to KV Generation creation, which is already internal UUIDv7 generation.
- Changes to the independent PostgreSQL query backend.
- New public insertion APIs, new dependencies, or changes to unrelated SQL and synchronization behavior.

## Further Notes

The investigation confirmed the current explicit UUIDv5/v7 contract and the connection among primary keys, Logical Row identities, document identities, and reconstruction actor derivation. It did not include a runtime reproduction of data corruption. Tests should establish the new external contract without claiming an unobserved failure mechanism.

The current monorepo history contains no earlier design rationale for explicit UUIDv5 support beyond the implemented public contract. Removing that contract is an intentional breaking change authorized by the no-compatibility decision.

## Comments

Implementation is integrated on `integration/generated-uuid-row-identities`. Full qualification is still open. See the individual issues for checks that passed, failed, or were not completed. Existing stores and histories require fresh initialization.

2026-11-24: All implementation and focused acceptance boxes are complete. TypeScript, browser, OpenAPI, owner-library, sync, and formatting checks passed. Full workspace feature-powerset, Clippy, and CRAP remain incomplete or unqualified for the infrastructure and pre-existing diagnostics recorded under issue 06; those broader results are not claimed as passes.
