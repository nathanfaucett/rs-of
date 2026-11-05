# Offline-first password manager: Bitwarden feature parity

Status: ready-for-agent
Date: 2026-11-05
Type: spec

## Problem Statement

The password manager has a local encrypted login workflow, but it is not yet a practical replacement for Bitwarden. Users need familiar item management, safe migration, reliable recovery, and offline use. They must not need an account or a reachable server to use their local vault.

The earlier gap analysis is not a verified comparison. The browser inspection reached the local vault selector, unlock screen, and create screen. The reference site redirected to login and identified itself as Vaultwarden Web 2026.6.4. Neither unlocked vault was inspected. Claims about full parity, encryption equivalence, tags, platform features, server compatibility, and delivery dates must not be treated as established facts.

This spec replaces those assumptions with a scoped product plan. It authorizes later implementation work; publishing it does not change the application.

## Solution

Build an offline-first password manager with familiar Bitwarden personal-vault workflows. Start with data safety and a verified feature baseline. Then add item types, navigation, daily-use tools, and migration. Treat platform integration, remote sharing, and synchronization as separate follow-on work.

### Evidence and current gaps

| Area                       | Verified current state                                                                                  | Required outcome                                                                                                  |
| -------------------------- | ------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| Vault lifecycle            | Named local vaults; create, select, unlock, close                                                       | Reliable local authentication, isolated vaults, settings, and automatic lock                                      |
| Encryption                 | Web Crypto PBKDF2-SHA-512, 600,000 iterations; AES-256-GCM per-record encryption                        | Review and test the complete storage/key lifecycle; algorithm choice alone is not a security assessment           |
| Empty-vault authentication | Unlock verifies a password only when a stored secret exists                                             | Wrong passwords fail for both empty and populated vaults                                                          |
| Vault identity             | Database names are derived from normalized display names                                                | Distinct vault identities; reject or safely handle name collisions without overwriting data                       |
| Items                      | One login-like Secret schema with a required password                                                   | Typed Login, Card, Identity, Secure Note, and SSH Key records with type-specific validation                       |
| Login data                 | Username, TOTP key, multiple URIs and match settings, notes, favorites, custom fields, password history | Preserve these fields; distinguish stored URI settings from working matching/autofill                             |
| Organization               | Favorite flag; existing workflow sorts by favorite and name                                             | Search, type/favorite/folder filters, folders, and sort controls                                                  |
| Daily tools                | TOTP key storage; no code generation in the editor                                                      | TOTP display/copy, password/passphrase generator, safe copy/reveal actions                                        |
| Deletion                   | Confirmed permanent deletion; optional deletedAt schema field                                           | Trash, restore, and separately confirmed permanent deletion                                                       |
| Backup                     | API serializes encrypted records but omits derivation salt; no restore path shown                       | Self-contained, authenticated backup and tested restore into clean storage                                        |
| Write safety               | Local collection validates complete snapshots and reconciles persisted writes                           | Retain failure handling; do not claim success on unconfirmed or partial writes                                    |
| Offline operation          | Local storage and local vault APIs exist                                                                | Prove cold launch/reload and core operations offline; local data alone does not prove offline application loading |
| Reference parity           | Unlocked reference workflows were not observed                                                          | Evidence-backed matrix before claiming parity or finalizing reference-specific details                            |

### Delivery order

The following work packages are the proposed ticket boundaries. They are not duration estimates. Create child tickets from these boundaries rather than copying the earlier PW numbers or week estimates.

| Order | Work package                                                     | Depends on | Completion gate                                                                                                                         |
| ----- | ---------------------------------------------------------------- | ---------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| A     | Verify personal-vault reference workflows                        | None       | Record observed behavior, reference version, evidence limits, and feature disposition without exposing account data                     |
| B     | Correct vault authentication, identity, and backup/restore       | None       | Empty-vault wrong-password rejection; collision safety; complete backup restored in clean storage; existing data preserved              |
| C     | Typed item model and safe format transition                      | A, B       | All five item types save and reopen; existing login data transitions without loss; failed transition leaves the source usable           |
| D     | Vault navigation, item detail, folders, search, filtering, trash | C          | Users can find, organize, restore, and safely delete items without a network                                                            |
| E     | Generator, TOTP, clipboard, history, and lock settings           | B, C       | Tools work locally; known-answer TOTP tests pass; lock removes access to decrypted state                                                |
| F     | Bitwarden import/export                                          | A, B, C, D | Supported fields round-trip through synthetic fixtures; unsupported data is reported; failed import does not partially modify the vault |
| G     | Offline startup and release qualification                        | B–F        | Previously loaded app starts and reloads offline with real local persistence; core acceptance flows pass                                |
| H     | Extended personal-vault features                                 | A, G       | Separate acceptance scope for encrypted attachments and local password-health reports                                                   |
| I     | Other clients and connected features                             | G          | Separate specs for autofill clients, passkeys, sync, sharing, and account features                                                      |

A is a bounded verification task, not permission to change either live vault. Reuse existing browser tabs. Do not enter, copy, reveal, export, or log real credentials. If the existing sessions are locked, record the evidence limit rather than opening replacement tabs or guessing.

## User Stories

The core release covers stories 1–30. Stories 31–36 define follow-on scope, not core release blockers.

1. As a vault owner, I want to create a local vault without an account, so that a server outage cannot stop me from starting.
2. As a vault owner, I want to unlock an empty or populated vault with my master password, so that authentication is consistent.
3. As a vault owner, I want wrong passwords to fail without changing my vault, so that failed access cannot damage my data.
4. As a vault owner, I want separately named vaults to remain isolated, so that creating one cannot overwrite another.
5. As a vault owner, I want to close and reopen my vault offline, so that local access does not depend on remote identity services.
6. As a vault owner, I want to reload the previously loaded app offline, so that local records remain usable without the development server or a remote host.
7. As a vault owner, I want manual and inactivity-based locking, so that unattended sessions stop exposing decrypted items.
8. As a vault owner, I want lock settings to persist, so that I do not need to configure them each session.
9. As a vault owner, I want to store Login items with optional credentials and multiple websites, so that the item can represent passwordless or username-only accounts too.
10. As a vault owner, I want Card items, so that payment details have clear fields instead of a generic password form.
11. As a vault owner, I want Identity items, so that contact and identity details stay together.
12. As a vault owner, I want Secure Note items without a required password, so that I can keep private text.
13. As a vault owner, I want SSH Key items, so that I can store public and private key material without pretending it is a login.
14. As a vault owner, I want notes, favorites, and applicable custom fields to survive edits and imports, so that I do not lose supporting information.
15. As a vault owner, I want a separate item detail view, so that reading or copying an item does not start an edit.
16. As a vault owner, I want explicit reveal and copy actions, so that passwords are not displayed by default.
17. As a vault owner, I want to cancel edits without changing stored data, so that viewing and experimentation are safe.
18. As a vault owner, I want search across item names, usernames, websites, notes, and custom-field names, so that I can find an item quickly.
19. As a vault owner, I want type, favorite, and folder filters plus clear sort controls, so that a large vault stays usable.
20. As a vault owner, I want to create, rename, and remove folders and move items, so that I can organize my vault without deleting items by mistake.
21. As a vault owner, I want deleted items to enter Trash, so that I can recover mistakes.
22. As a vault owner, I want to restore or permanently delete a trashed item, so that I control its retention.
23. As a vault owner, I want configurable random passwords and passphrases, so that I do not need an external generator.
24. As a vault owner, I want locally generated TOTP codes and a countdown, so that I can use stored authenticator keys.
25. As a vault owner, I want dated password history, so that I can inspect an earlier password when needed.
26. As a vault owner, I want a self-contained encrypted backup, so that loss of browser storage does not mean loss of my vault.
27. As a vault owner, I want to restore a backup in a clean browser profile, so that recovery does not depend on hidden source storage.
28. As a Bitwarden user, I want to preview an import and see unsupported fields before it writes, so that migration cannot silently discard data.
29. As a vault owner, I want a compatible export of supported Bitwarden fields with a plaintext warning, so that I can leave the application safely.
30. As a vault owner, I want clear errors for corrupt files, full storage, failed writes, and uncertain outcomes, so that I know whether to retry or recover.
31. As a vault owner, I want encrypted attachments, so that supporting files can be retained with an item.
32. As a vault owner, I want local reused-password and weak-password reports, so that I can improve security without sending credentials away.
33. As a vault owner, I want optional online breach checks with explicit consent, so that I understand the privacy and network cost.
34. As a vault owner, I want browser and mobile autofill, so that credentials are usable outside the web vault.
35. As a vault owner, I want optional synchronization that preserves local writes during a network failure, so that adding another Device does not remove offline operation.
36. As a vault owner, I want sharing and account access controls defined separately, so that local vault unlock is not confused with remote authorization.

## Implementation Decisions

### Product boundary and terminology

- A local vault is an encrypted application data set, not a Bitwarden organization, an Installation, a Node, or an authenticated User account. These are not interchangeable concepts.
- Keep the current local vault/collection/workflow ownership boundaries. Reuse existing storage and UI libraries. Add no dependencies during spec publication.
- Feature parity means selected user-visible workflows and supported data semantics. It does not mean identical pixels, identical cryptography, or Bitwarden/Vaultwarden wire-protocol compatibility.
- Reference-only features require evidence from the selected deployment or official product documentation before they become committed parity requirements. Record hosted/deployment/client/version restrictions.
- Tags, rich-text editing, custom accent colors, exact keyboard bindings, entropy displays, and a printable emergency sheet are not confirmed baseline requirements.

### Local storage and security

- Keep core vault operations independent of IdP, Management, Storage services, or any remote API. Future integration must retain the project ownership boundaries.
- Authenticate the master password independently of whether secrets exist. Reject wrong passwords before showing an unlocked state.
- Use stable vault identity separate from display names. Creation must not overwrite an existing vault through case or punctuation normalization.
- Keep item contents, folders, and sensitive settings encrypted at rest. Do not add plaintext full-text indexes, password-health indexes, or search caches to persistent storage.
- Search the unlocked in-memory view first. Do not prescribe FTS5 or a new indexing service without measured need.
- Lock must retire the active workflow, deny further operations through the old handle, and release application references to keys, records, drafts, generated values, and history. Do not claim guaranteed JavaScript memory zeroization or protection against a compromised browser.
- Define pending-write behavior on lock. Do not allow late asynchronous results to repopulate a locked view or claim a cancelled write was saved.
- Keep persisted-write acknowledgement and complete-snapshot validation. On uncertain outcomes, reload authoritative local records and tell the user to verify before retrying.
- Backup format must include version, derivation parameters, salt, and all material needed to authenticate and restore. Authenticate backup contents and metadata. Never export an unprotected key.
- Master-password change, alternative KDF selection, and recovery-key design need separate security decisions; do not bolt them onto this work without review.

### Item model and organization

- Replace the single mandatory-password schema with a discriminated item model. Login, Card, Identity, Secure Note, and SSH Key each have relevant fields and validation. The exact reference field mapping is a deliverable of A and C.
- Preserve existing login URIs, match settings, custom field types, favorite state, notes, timestamps, and history. A stored URI rule is not proof of matching or autofill implementation.
- Linked custom fields and other unsupported reference field variants require explicit import disposition; do not silently convert them to ordinary text fields.
- Secure Note starts as private text. Markdown preview and rich text are not needed for the core release.
- SSH Key storage does not imply key generation, SSH agent operation, or passkey support.
- Folders are vault-local. Folder deletion must leave contained items accessible as unfiled items after explicit confirmation. Reference nesting semantics must be verified before claiming nested-folder parity.
- Trash hides deleted items from normal results. Restore retains the item identity and data. Permanent deletion requires separate confirmation. Any automatic retention period requires a stated policy and tests; do not purge based on an assumed 30-day rule.
- Bulk mutation support must use an atomic persistence boundary or a clearly specified recovery mechanism. The current collection accepts only one secret mutation per operation; an import must not use that loop and claim atomic success.

### Tools and settings

- Generate passwords and passphrases with cryptographically secure randomness. Validate impossible settings. Keep generated history out of persistent plaintext storage.
- Generate TOTP locally from supported raw secrets and otpauth URIs. Validate algorithm, digits, and period; report unsupported inputs. Account for time-based updates and resume from a suspended tab.
- Do not use ordinary client-side TOTP checks as a claimed second cryptographic factor for offline vault decryption. Remote account MFA and local key release are different designs.
- WebAuthn authentication alone does not establish a usable biometric decryption-key mechanism. Biometric/PIN unlock remains separate work.
- Provide keyboard-operable controls, labels, focus handling, masked sensitive fields, copy failure feedback, and unsaved-edit handling.
- Use an inactivity deadline, checked on activity and tab resume, rather than trusting a background timer alone. Automatic lock cannot promise operating-system lock detection in an ordinary browser.
- Clipboard clearing is best effort. Do not overwrite newer clipboard content or claim copied secrets are guaranteed removed.

### Import, export, and data transitions

- Start with a verified, explicit Bitwarden JSON subset. Use synthetic exports and list each supported item and field mapping. CSV and other vendors are follow-on formats, not hidden requirements of the first importer.
- Preview counts, validation failures, unsupported data, and duplicate handling before commit. Never silently drop unknown fields, attachments, passkeys, organization ownership, or collection permissions.
- Import to a chosen local vault; do not imply that imported organization metadata recreates shared access or roles.
- Treat repeated imports explicitly: show duplicate choices or reject ambiguous matches; do not silently overwrite by display name.
- Validate the full input before writes. Commit atomically or leave the destination unchanged and show an actionable error.
- Export only on explicit action. Warn that plaintext formats expose credentials. Do not put secrets in logs, telemetry, URLs, or notifications.
- Use a versioned format transition with backup and rollback/recovery checks. Preserve existing user data, then remove obsolete implementations; do not retain duplicate legacy writers or compatibility wrappers.

### Offline delivery

- Support offline application loading after the first successful load/install. A first-ever visit with no cached application is not an offline guarantee.
- Include application assets and required database/WASM assets in the offline delivery checks. Show clear failure if required resources are unavailable.
- Handle browser storage denial, quota limits, eviction risk, and private-browsing limits. Explain that browser-local persistence is not a backup.
- Future online reports, synchronization, and sharing must have explicit network states. Do not describe inherently connected operations as available offline.

## Testing Decisions

The user approved these boundaries before publication:

1. **Primary boundary: user-visible application flows.** Test create, unlock, view, edit, search, organize, import, export, restore, and lock through the application. Use the real workflow and collection. Reuse the existing mounted page/editor test patterns. Prefer assertions on visible behavior and persisted outcomes over internal calls or component structure.
2. **Supporting boundary: public vault API with real persistence.** Reuse the existing create/store/close/reopen test style. This boundary establishes encryption, authentication, durable writes, damaged-data handling, and recovery. UI mocks are not evidence for these properties.
3. **Release qualification: real browser offline flows.** Launch a previously loaded app with network access disabled, reload it, unlock it, change records, lock, and reopen. Use test-only vaults and clean-profile backup restoration. Mock persistence tests do not establish offline startup.

No new cross-cutting test abstraction is required. Add focused known-answer tests for generators/parsers and TOTP when the application boundary cannot precisely isolate the rule.

### Acceptance cases

- Correct and wrong passwords against empty and populated vaults; wrong-password attempts leave data unchanged.
- Case/punctuation name collisions and distinct vault isolation; creation never destroys an existing vault.
- Each item type can be created, viewed, edited, cancelled, and reopened offline. Notes and non-login items do not require a login password.
- Existing login data survives the format transition with nested URIs, fields, dates, favorites, and history intact. Failed transitions leave a recoverable source.
- Search/filter/sort combinations, empty results, unfiled items, folder rename/delete, and cancelled actions produce the expected visible records.
- Search never requires persisted plaintext item content. Storage inspection uses synthetic recognizable values to check sensitive fields and backup payloads.
- Trash/restore/permanent-delete behavior persists across reopen. Normal lists exclude Trash; permanent deletion affects only the confirmed items.
- TOTP matches known vectors, respects supported digit/period/algorithm values, updates at a boundary, and handles malformed URIs without exposing the key in errors.
- Generator output obeys selected constraints and rejects invalid configurations. Do not use flaky statistical tests as proof of cryptographic security.
- Manual and automatic lock hide all decrypted views, retire stale handles, handle pending writes, and prevent late results from restoring the session. Timer tests include suspended/resumed tabs.
- Export a complete encrypted backup, discard the source environment, restore in clean storage, and compare all supported logical data. Wrong passwords, missing metadata, tampering, unsupported versions, and truncation fail safely.
- Synthetic Bitwarden fixtures cover all supported item/field mappings, Unicode, multiline text, missing optional fields, unknown variants, duplicate import, and round-trip export. Reject or report unsupported data before writes.
- Import/restore failures and quota/write failures do not leave a partial destination or a false success message. Preserve current authoritative-reload behavior for uncertain writes.
- Reload/start offline with required assets available, then complete the core workflows without any successful network request. Online-only actions show their unavailable state without blocking local access.
- Keyboard and focus tests cover dialogs, navigation, item actions, and error feedback. Test clipboard permission failure and best-effort clearing without relying on clipboard availability.

Use focused checks during implementation and the existing application test/build commands for release validation. Any storage-library changes must also run the affected Rust repository's required validation. No tests are claimed as run by this spec.

## Out of Scope

For this spec publication:

- Application implementation, dependency installation, data changes, live vault exports, or credential entry.
- Creating a full set of child tickets before the reference baseline and security contracts are checked.
- Treating the earlier gap analysis estimates as approved commitments.

For the core offline personal-vault release:

- Bitwarden/Vaultwarden server compatibility or synchronization protocol implementation.
- Organizations, shared collections, roles, groups, policies, SSO, SCIM, event logs, billing, and remote account management.
- Emergency access and Send-style sharing. Trusted timers and remote delivery need separate designs.
- Browser extension, native mobile/desktop client, CLI, local HTTP API, SSH agent, and passkey provider implementation.
- Biometric/PIN unlock, vault MFA, password recovery, master-password change, and cryptographic redesign without separate review.
- Attachments, health dashboards, online breach checks, other import vendors, CSV, tags, rich-text editing, and optional visual customization. These remain follow-on candidates.
- Plaintext persistent search indexes or automatically sending vault contents to third parties.
- A fixed delivery date, staffing assumption, or blanket claim of full Bitwarden parity.

## Further Notes

### Source context

- Initial request: compare `https://password-manager.localhost:1355/` with `https://bitwarden.nathanfaucett.com/#/vault` using Playwriter; plan an offline-first Bitwarden-like application without implementation changes.
- Browser constraint: reuse existing tabs. The earlier inspection opened tabs and did not complete an unlocked comparison. Future verification must honor the reuse constraint.
- This spec replaces the deleted gap analysis and is the acceptance authority for this effort.
- Local code evidence: `apps/password-manager/src/lib/vault/vault.ts`, `apps/password-manager/src/lib/secrets/secret.ts`, and `apps/password-manager/src/lib/secrets/local-collection.ts`.
- Existing test patterns: `apps/password-manager/src/routes/page.test.ts`, `apps/password-manager/src/lib/secrets/editor.test.ts`, and `apps/password-manager/src/lib/vault/vault.test.ts`.
- Domain vocabulary: `GLOSSARY.md` and `GLOSSARY-MAP.md`. This local-vault plan does not redefine User, Device, Node, Installation, or resource authorization.

### Corrections carried forward

- Multiple local vaults are not equivalent to organizations or shared collections.
- URI match settings do not establish an autofill implementation.
- TOTP secret storage is not code generation.
- An encrypted-record export without required derivation metadata and a proven restore is not a complete backup.
- A local database is not proof that the application starts offline.
- The earlier claims about tags, customizable accents, exact shortcuts, API/SDK scope, encryption equivalence, and timetable were not verified.
- Bitwarden client surfaces and Vaultwarden support must be distinguished. Do not use the selected web vault as proof of native-client or hosted-enterprise behavior.
- Password breach checks and email breach reports have different disclosure, consent, provider, and possible cost requirements. An HTTP URI report does not prove that an account has inactive MFA.
- Do not claim lossless migration for attachments, passkeys, or any other unmapped data. Narrow the claim to the tested supported subset and report exclusions.

### Complexity, risk, and priority

- Complexity: high overall; keep work split at the existing vault, collection/workflow, and user-interface boundaries.
- Highest risks: data loss, unusable backups, unauthenticated empty vaults, name collisions, plaintext leaks, and false offline guarantees.
- Priority: complete B before using real migrated credentials; complete A before claiming parity; complete G before calling the product an offline-first replacement.
- Resolve security and data-format questions in the affected work package before implementing them. Stable design/operation decisions belong in named topic documents, not new ADRs.

## Comments

- 2026-11-05: User approved the application-level test boundary, supporting real-persistence vault tests, evidence corrections, publication location, and ready-for-agent status. No delivery estimates or wire-protocol compatibility were approved.
