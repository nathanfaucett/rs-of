# App-owned data map

Status: needs-triage

## Notes

- [Proposed spec](spec.md). Planning only; no implementation or runtime acceptance.
- Scope: password manager and its collection first; service conversion comes later.
- Triage uses `Status:`. Work state uses `Work status:` and starts `open`.
- Claim the lowest-numbered open, unblocked ticket when implementation is approved.

## Decisions-so-far

Proposed direction, subject to review:

- Local app data access must not require an Installation, account, desktop host,
  or server. A local app store is not a Node.
- Keep SQL for the first app. Reuse public ofdb; do not emulate remote SQL locally.
- Keep the app's schema, official TanStack binding, validation, and atomic writes.
- Prove the app store before extracting the collection or SDK setup.
- Rewrite the unused byte-backend collection interface instead of preserving it.
- Encrypt before persistence. Existing remote data stays untouched.
- Keep sync optional and separate from local save/unlock. Reuse owner sync protocols.
- Leave current system design/glossary intact until the proof supports a new
  service plan. This spec explicitly records the conflict.

## Work

| Ticket                                | Output                                                                        | Blocked by |
| ------------------------------------- | ----------------------------------------------------------------------------- | ---------- |
| [01](issues/01-browser-bindings.md)   | Qualified SQL JS/WASM contract and reproducible artifacts                     | —          |
| [02](issues/02-local-vault.md)        | App-owned encrypted local vault and reopen/recovery tests                     | 01         |
| [03](issues/03-local-collection.md)   | Focused local-source collection; old adapters deleted                         | 02         |
| [04](issues/04-app-startup.md)        | Browser-only app startup, local lock, optional account setup                  | 03         |
| [05](issues/05-browser-acceptance.md) | Real-browser offline, failure, and recovery evidence                          | 04         |
| [06](issues/06-sync-contract.md)      | Protocol draft, two-store proof, and later service-plan input (blocked on 05) | 05         |

## Fog

Resolve each question in its owning ticket, not with a general framework:

- 01: exact lossless JS value format, generated binding signatures, handle
  disposal, worker requirements, WASM stack, and package artifact distribution.
- 02: reviewed vault format/crypto dependency, local creation evidence, encrypted
  export format, conflict recovery, and schema evolution rules.
- 03: smallest source interface that preserves atomic commit and view publication.
- 04: which setup work actually warrants `@of/sdk`; static asset offline lifecycle.
- 05: supported browsers and measured storage/size limits; backup and loss UX.
- 06: browser transport, store/replica identity, authorization/enrollment without
  requiring a browser Node, key distribution, and retained encrypted conflicts.

## Evidence limits

Source and documentation inspection only. Existing ofdb Rust browser evidence is
not app/TypeScript/sync-backend acceptance. No service conversion is authorized by
this plan. All tickets need review before implementation.
