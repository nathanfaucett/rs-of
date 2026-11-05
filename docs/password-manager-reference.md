# Password manager reference baseline

Research date: 2026-11-05. Acceptance authority: [spec](../.scratch/password-manager-bitwarden-parity/spec.md), [ticket 01](../.scratch/password-manager-bitwarden-parity/issues/01-reference-baseline.md).

This is a scoped personal-vault baseline, not a claim of full Bitwarden parity. The field and portability contract is in [Password manager JSON](password-manager-json.md). No application behavior changes with these documents.

## Evidence and limits

| Evidence | Result | What it does not establish |
| --- | --- | --- |
| Current Playwriter session 11, group `baseline` | Extension connected. `context.pages()` returned an empty array, including an unfiltered second enumeration. No enabled existing tab was available. | Neither target page, login state, version, nor unlocked behavior could be inspected. This is a tab-access limit, not proof that either service is down or locked. |
| Earlier inspection, recorded in the approved spec | Local selector, unlock, and create screens were reached. Reference redirected to login and displayed **Vaultwarden Web 2026.6.4**. | This is inherited evidence, not a version observed again in this task. It identifies a web client, not the server version. Neither unlocked vault was inspected. |
| Local source at `42391ef3c943365a2319b8118215e333d28d3e44` | See source matrix below. | Source inspection is not browser acceptance or a security audit. The existing tab's build revision is unknown. |
| Official Bitwarden help [H1–H7] | Product workflows and restrictions below. | Hosted Bitwarden documentation does not prove that the selected Vaultwarden deployment enables the same features. |
| Official Bitwarden client source [S] | Export keys and enum values; pinned to `ab8ea7db118f6f0b4878014594c5d822a658dbe0`. | Upstream development source is not the selected deployed release. It contains newer types beyond this spec. |
| Vaultwarden project wiki [V1] | Unofficial compatible server; supported features and configuration limits. | The selected server version, configuration, retention, and feature flags remain unknown. |

Browser safety: no tab was created, navigated, closed, unlocked, or changed. No password was entered. No live item, credential, clipboard, export, cookie, storage, screenshot, network payload, or page log was read. The Playwriter skill was read in full before enumeration. Only tab metadata was requested; there were no pages to inspect. No replacement browser was started. Background research tools were unavailable, so primary-source research ran in the assigned worktree.

### Local source comparison

Paths below are relative to the repository root. These findings apply to the starting revision, not later integration work.

| Source | Established behavior / gap |
| --- | --- |
| `apps/password-manager/src/lib/secrets/secret.ts` | One Secret schema. UUID and nonempty name/password required. Optional username/TOTP/notes, ordered URIs, favorites, text/hidden/boolean fields, dated password history, creation/update dates, optional deletion date. No item discriminator, Card, Identity, Secure Note, SSH Key, folder, or linked-field model. URI omission defaults to `domain`; it cannot represent inherited `null` separately. |
| `apps/password-manager/src/lib/secrets/secret-draft.ts` | Draft copies URI and field arrays. Save removes blank URI rows, retains creation date, updates modification date, and appends the prior password when it changes. This is not proof of a reference history limit or of history display. |
| `apps/password-manager/src/lib/secrets/local-collection.ts` | Loads and validates complete snapshots. Rejects malformed or mismatched records without loading a partial set. Supports only one secret mutation per transaction. Reconciles persisted results and reports unacknowledged/uncertain writes. No atomic multi-item import contract exists. |
| `apps/password-manager/src/lib/vault/vault.ts` | PBKDF2-SHA-512, 600,000 iterations, AES-256-GCM, random salt/nonces. Empty-vault unlock skips password verification. Database names normalize display names. Permanent SQL deletion. `exportEncrypted` omits the salt and has no restore operation. `close` closes clients, but does not prove stale-handle or memory cleanup. |
| Spec's earlier UI findings | Named vault selection/create/unlock/close and favorite/name ordering. These were not re-observed here. No unlocked UI or offline startup evidence is added by this ticket. |

## Workflow and feature dispositions

**Core** means required by this product spec, not already implemented. **Follow-on** means separate future scope. **Unsupported** means excluded from the first contract. **Unverified** means no dependable baseline requirement yet. An evidence limit does not remove a core product requirement.

All deployed-reference workflows below remain unobserved in this task.

| Candidate | Reference evidence and restrictions | Local baseline / product decision | Disposition |
| --- | --- | --- | --- |
| Local create/select/unlock/close, isolated vaults | Bitwarden accounts and personal vaults are not local database selectors. | Keep account-free local operation; fix empty-vault authentication and name collisions before real migration. | Core |
| Five item types; create/read/edit/cancel | H1 lists Login, Card, Identity, Secure Note, SSH Key; S gives exact fields. | Replace mandatory-password Secret with typed data. No password required for non-login types or optional-credential Login. | Core |
| Notes, favorites, custom fields | H2: text, hidden, checkbox/boolean, linked; S gives numeric variants. | Preserve ordered fields, Unicode, multiline text; support first three variants. Linked fields must block initial import rather than become text. | Core (first three); unsupported (linked) |
| URI list and rule selection | H3: default, base domain, host, starts with, exact, regex, never; platform behavior differs. | Preserve rules and order, including inherited default. Stored settings are not matching or autofill. | Core storage; follow-on execution |
| Separate detail/edit, masked values, explicit copy/reveal | H1 documents copy and edit surfaces. | Keyboard access, cancel, clipboard failure feedback, best-effort clear; no guarantee of browser memory zeroization or clipboard removal. | Core |
| Search/type/favorite/folder filters and sorting | H1 points to search/navigation; H4 describes folders. Exact search syntax/order is not established here. | Search unlocked memory across name, username, URI, notes, custom-field names. No plaintext persistent index. Clear sort controls; no exact UI-copy requirement. | Core |
| Folder create/rename/delete/move | H4: alphabetical personal folders; deletion keeps items; folders are not shared collections. | Vault-local folders; deleting one leaves items unfiled after confirmation. | Core |
| Nested folders | H4: slash-delimited names nest only when parent exists; parent search excludes children; deleting parent does not delete child folders. | Preserve full names and independent folder IDs. Hierarchical UI and cascading rename are not core promises. | Follow-on UI; core name preservation |
| Trash/restore/permanent delete | H1: Bitwarden retains 30 days. V1: Vaultwarden retention is configurable. Selected setting unknown. | Hide Trash from normal results. Restore same identity/data. Separate permanent-delete confirmation. **No automatic purge in core**; never assume 30 days locally. | Core |
| Password history | S: ordered password/last-used-date pairs. Local draft appends prior passwords. Reference UI limit and all triggers unverified. | Preserve supplied entries/dates without truncation. Record changed prior login password; support dated inspection. Do not equate last-used with last-login telemetry. | Core |
| Password/passphrase generator | Spec requires local secure random generation. Exact deployed options not inspected. | Validate impossible settings; no persistent plaintext generated history. | Core |
| TOTP seed and code display/copy | H7: key storage available to all; code generation Premium/paid organization on Bitwarden. V1 supports TOTP; selected entitlements unknown. H7 default SHA-1/6 digits/30 seconds and configurable parameters. | Local code generation is core, independent of remote billing. Initial computation: raw Base32 or `otpauth://totp`, SHA-1/256/512, 6 or 8 digits, positive integer period. Preserve other strings but mark code generation unavailable; Steam/HOTP/other settings are follow-on. Not a second factor for local decryption. | Core subset; follow-on extensions |
| Lock/manual timeout settings | Product spec requires manual/inactivity lock, persisted settings, resume deadline checks, pending-write handling. Deployed behavior not inspected. | Retire handles and references to decrypted state. Ordinary web app cannot promise OS-lock detection or secure JS erasure. | Core |
| Encrypted backup/clean restore | H6 distinguishes account-restricted and password-protected exports. Current local export is incomplete. | Separate self-contained authenticated local backup; do not call encrypted records or Bitwarden plaintext export a complete backup. | Core |
| Bitwarden personal JSON import/export | H5 and H6; JSON includes more types than CSV; export omits Trash/Sends. | Strict [supported subset](password-manager-json.md), preview, explicit exclusions, duplicate policy, atomic write. No live exports used. | Core subset |
| Offline cold reload and persistence | No fresh browser evidence. Bitwarden's logged-in TOTP offline claim (H7) is not proof of this app's asset caching. | Previously loaded application/assets plus database/WASM must start offline. Clean-storage recovery remains a release gate. | Core, unverified implementation |
| Attachments | H5: upload separately for import. H6: individual ZIP with attachments is a distinct export. V1 supports attachments. | Encrypted attachments need separate storage/recovery work; reject attachment-bearing initial JSON. | Follow-on; unsupported initial JSON |
| Local weak/reused password reports | Spec follow-on scope; no deployment inspection. | No plaintext health indexes. | Follow-on |
| Breach checks | Network/provider/privacy requirements not verified. | Explicit consent, network and cost disclosure before any remote check. | Follow-on |
| Autofill extension/mobile/desktop/CLI/API | H3 and H8 are client-specific; ordinary web vault is not an autofill provider. | Separate clients/specs; no claim from stored URIs. | Follow-on |
| SSH generation/agent | H1 mentions agent use, but that is not web storage parity. | Store SSH fields only; no agent, generation, or key usability guarantee. | Follow-on |
| Passkeys / credential exchange | H6: passkeys in JSON; CXP on iOS 26+/Android 10+ with compatible destination. | Reject nonempty passkey data; do not claim credential exchange support. | Follow-on; unsupported initial JSON |
| Sync/sharing/organizations/collections | V1 supports several configured server features. Personal folders do not grant access (H4). | Local vault is not an organization, Installation, Node, or account. No remote protocol/roles recreated on import. | Follow-on; unsupported core |
| Send/emergency access/SSO/SCIM/billing/account management | V1 differs from official hosted features; deployment settings unknown. | Separate connected-product scope. Not core personal-vault parity. | Unsupported core |
| PIN/biometric unlock/MFA/recovery/master-password change/new KDF | No reviewed local key-release or recovery design. | Separate security decisions; avoid claiming encryption equivalence. | Follow-on |
| Archive | Current H1 says paid plans; included in exports, excluded from search/autofill. Upstream S has `archivedDate`. Deployed support unknown. | Not in approved core. Reject non-null archive data initially, never silently unarchive. | Follow-on; unsupported initial JSON |
| New upstream Bank Account/Driver's License/Passport types | S enum adds 6/7/8, beyond H1's five-type help and the selected older version. | Do not extend scope from development source. Reject unknown types explicitly. | Unsupported initial JSON |
| Clone/bulk actions | H1 documents cloning and bulk deletion; clone omits passkeys. | No standalone clone requirement in spec. Bulk operations require atomic persistence, not single-record loops. | Follow-on UI; core atomic import |
| Tags/rich text/accents/exact shortcuts/entropy/emergency sheet | Not established by inspected sources or selected deployment. | No baseline commitment; plain text notes only. | Unverified |

## Sources

Help pages were read on 2026-11-05. They are moving product documentation, not version-pinned deployed behavior. A failed URL is not used as evidence.

- **H1** [Vault Items](https://bitwarden.com/help/managing-items/): five types, create/copy/edit, archive, clone, Trash.
- **H2** [Custom Fields](https://bitwarden.com/help/custom-fields/): four variants, name/value semantics, encrypted-size limit. The local contract does not adopt that encrypted 5,000-character ceiling.
- **H3** [Form URIs for Autofill](https://bitwarden.com/help/uri-match-detection/): default inheritance, six strategies, regex/prefix risks, equivalent domains, client restrictions.
- **H4** [Folders](https://bitwarden.com/help/folders/): personal organization, nesting, deletion and parent-search rules.
- **H5** [Import from a Custom File](https://bitwarden.com/help/condition-bitwarden-import/): case-sensitive keys, optional data, empty type objects, personal/organization distinction. Its examples show four types, not SSH; S supplies SSH fields.
- **H6** [Export Vault Data](https://bitwarden.com/help/export-your-data/): JSON/CSV/encryption/ZIP/CXP, omitted Trash/Sends, ownership restrictions. No export was performed.
- **H7** [Integrated Authenticator](https://bitwarden.com/help/authenticator-keys/): entitlements, offline logged-in use, defaults and custom parameters, Steam distinction.
- **H8** [Autofill Cards & Identities](https://bitwarden.com/help/auto-fill-card-id/): client limits. It limits Identity autofill to extensions, unlike the broader wording in H1; no Android Identity autofill claim is adopted.
- **V1** [Vaultwarden project wiki](https://github.com/dani-garcia/vaultwarden/wiki): unofficial server, configurable Trash, feature/configuration limits.
- **S** [Bitwarden client source](https://github.com/bitwarden/clients/tree/ab8ea7db118f6f0b4878014594c5d822a658dbe0): exact field source links in [JSON contract](password-manager-json.md#first-party-field-sources). The pin was obtained from GitHub's `commits/main` API. A source digest manifest accompanies the fixtures.

## Qualification still required

An enabled existing target tab is needed to inspect public screen labels/version safely. An unlocked synthetic-only vault is needed to exercise CRUD, folder, Trash, history and portability behavior without touching real data. No real-account session should be unlocked for this ticket. Selected server version/configuration, client options, deployed field validation, and exact UI workflows remain unverified. These limits do not block the documented five-type subset design; they do block a claim of observed deployment parity.

Application implementation, importer round-trip, clean-profile backup restoration, cryptographic review, and real offline browser flows remain later-ticket gates. Synthetic fixture checks establish only fixture structure, not a working importer or Bitwarden acceptance. No delivery estimate is made.
