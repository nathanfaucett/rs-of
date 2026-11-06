# Password manager encrypted folders

Ticket 12 adds vault-local folders. This is not an organization, shared collection, access policy, or claim of observed hierarchical parity. Naming follows the [personal JSON contract](password-manager-json.md#folders).

## User actions

Select **New folder**, enter a name, and save. Names must contain a non-whitespace character. Accepted names retain their exact text, including Unicode, spaces, and slashes. Duplicate names have separate UUIDs. The selector shows each full ID. Slashes do not create ancestors or trigger descendant rename/delete.

The folder selector offers **All folders**, **Unfiled**, and exact folder membership. Empty views have an explicit message. Open an item, choose **Move to folder**, and select **Move item**. Choose **Unfiled** to remove membership. Select a folder to rename or delete it. Deletion requires **Confirm folder delete**; cancel changes nothing. Deletion clears only direct membership, including records in Trash. It deletes no item and no other folder.

Folder forms participate in the editor's unsaved-change guard. Refusing navigation retains the draft and selected filter. Name entry and delete confirmation receive focus. Save/cancel returns focus to the folder selector. Manual lock always retires the form, selection, folder list, workflow, and vault handle. Late results cannot restore the view. A submitted write can have persisted: reopen and verify before retrying.

Folder filtering runs in `workflow.records`, before any caller's search/type/favorite/sort transformation. `detail` still resolves from the complete collection by ID. This retains one item authority and permits ticket 11's `searchItems(workflow.records, options)` to compose with exact folder membership without another collection or persistent index. Folder selection does not change the other filter settings. The integrated UI combines these controls. Normal results exclude Trash before folder and search filters. Trash is separate and unfiltered; returning to All items retains the normal filters.

## Public API and importer boundary

`src/lib/vault/folder.ts` exports strict `FolderSchema`, `Folder`, `ItemSnapshot`, `ItemMutation`, and `validateItemSnapshot`.

- `Folder`: `{ id: UUID, name: nonblank string }`.
- `ItemSnapshot`: `{ items: Secret[], folders: Folder[] }`.
- `await vault.readItems()`: prepare the typed format and return one complete, validated snapshot from the current published database. Duplicate IDs, invalid decrypted records, inner/outer ID mismatches, or dangling memberships reject the whole snapshot.
- `await vault.mutateItems({ upsert?, deleteIds?, folders?, deleteFolderIds?, expectedSnapshot?, expectedItems? })`: merge item/folder upserts and removals into the authoritative snapshot under the vault's Web Lock. `folders` means upserts, not replacement. Duplicate batch IDs or an ID both removed and upserted are rejected. Optional `expectedSnapshot` must equal the complete current snapshot. Optional `expectedItems` entries contain `{ id, record }`; each record must equal the current item, or be null for a new identity. A guarded removal also requires current Trash state. These checks run under the same item lock, before staging, and reject with refresh guidance. Independent changes to other items still merge for record-guarded writes. Validate the complete result before persistence. Deleting a referenced folder through this batch API requires explicit item updates in the same batch; otherwise validation rejects it.
- `await vault.createFolder(name)`: return the committed folder with a fresh UUID.
- `await vault.renameFolder(id, name)`: retain the folder ID and memberships; reject a missing folder.
- `await vault.moveItems(ids, folderId | null)`: move all named items together; reject missing/duplicate item IDs, an empty selection, or a missing destination.
- `await vault.deleteFolder(id)`: remove the folder and set direct members' `folderId` to null in the same operation. Retain all other item data, including deletion dates. Membership changes update item `updatedAt`.

The importer must validate/preview its entire input, allocate fresh local IDs, remap every folder reference, and call `mutateItems` once with all folders and items. Do not loop over single-item writes. Do not merge duplicate names or use external IDs as overwrite authority. This API does not implement import preview, duplicate policy, or external-format validation.

The collection loads `readItems()` and publishes folders with its complete validated item snapshot. Existing single-item writes after preparation also use the atomic mutation boundary. Collection acknowledgement and authoritative reconciliation remain in place. Low-level unprepared encrypted-record access remains for the existing storage/recovery boundary, not as an importer writer.

## Persistence and failure safety

SQL migration **4** creates `folders` with ID, ciphertext, nonce, and outer timestamps. Migration versions 1–3 are unchanged. Folder names are inside per-record AES-GCM ciphertext. Item membership is inside the encrypted item. There is no plaintext name, tree, membership, or search index.

A mutation holds the existing `password-manager-items-<vault ID>` Web Lock across authoritative load, validation, encryption, staging, verification, and publication. It reuses backup restore's `stageVault`: one SQL transaction writes complete metadata/proof, item format, retained transition backup, items, folders, and exact encrypted lock settings into an unlisted `password-manager-batch-<UUID>` database. The database is closed and reopened; exact ciphertext, nonces, IDs, timestamps, metadata, proof, and format are compared before publication.

Only then does the registry compare the original entry and publish the complete database pointer in one IndexedDB transaction. Registry readback is required. Other handles resolve the current pointer under the same lock before writing. The original pre-transition database reference is retained across later mutations, so it is not rediscovered as a second vault.

A failure before publication leaves the source unchanged. Publication/readback failure can leave either the old complete snapshot or the new complete snapshot visible, never an intentionally published subset. The API reports uncertainty, not success. Refresh/reopen reloads the authoritative pointer. Verify before retrying. No error path clears storage or assumes a rejected durable write rolled back.

**Space limit:** each mutation stages a complete vault and retains old encrypted databases. There is no automatic cleanup. This favors the existing verified recovery protocol over an unverified in-place multi-record write. Large vaults and repeated writes can use substantial storage; keep an external backup and check free space. This is not secure deletion or a JavaScript memory-zeroization guarantee.

## Backup version 3

Current typed-vault exports use envelope **version 3**, still `itemFormat: 2`. The envelope version is included in AES-GCM additional authenticated data. The payload adds required `folders`, an encrypted-record array using the same outer fields as `secrets`. Empty folders are included. Every folder is decrypted and checked for matching identity; the complete item/folder relationship is validated before export or restore preparation succeeds.

Versions 1 and 2 remain readable with their original meanings. They cannot contain `folders`; version 3 must contain it and must use typed items. Embedded pre-transition backups remain version 1 and cannot contain folder data. Old version 2 files with a dangling historical membership can be restored for recovery but cannot load as a valid current complete item/folder snapshot; no folder identity is guessed or silently cleared.

Restore stages and verifies exact encrypted folders and lock settings as well as items. The existing 16 MiB total backup limit includes folders and retained recovery bytes. Each of the item and folder arrays is limited to 10,000 records. Preparation/preview `recordCount` counts items, not folders. No key, folder name, or item membership is exposed in the outer backup envelope.

## Integration and evidence

The combined migration list uses 4 for folders and 5 for encrypted lock settings. Version 3 backups retain both. Every mutation and format-transition staging payload preserves settings and folders, with exact reopen verification. Restore from Trash retains membership when the folder exists; folder deletion atomically unfiles active and trashed members.

Evidence uses synthetic data only:

- `src/lib/vault/folders.test.ts`: literal/duplicate names, direct-only deletion including Trash, exact clean-storage recovery, encrypted names, rejected/lost writes, rejected/uncertain move publication, complete-snapshot rejection for damaged folder identity, invalid batch rejection, concurrent/stale handles, pending lock, reopen, and vault isolation.
- `src/routes/folders.test.ts`: mounted real page/workflow/SQL, form and deletion cancellation, focus, refused navigation, failed move/retry, rename, exact/unfiled/empty views, deletion, lock/reopen, and persisted membership.
- Existing editor, collection, transition, backup, identity, authentication, and detail suites retain their acceptance checks. The late acknowledgement test gates the public write result after actual persistence, rather than depending on the old crypto call order.
- `scripts/check-backup.mjs`, called by `check-offline.mjs`: built offline folder creation/move/filter/rename, deletion cancellation, empty-folder preservation, source-profile disposal, clean-profile recovery, and exact complete encrypted payload comparison.

Other browser engines, actual disk exhaustion, independent security review, remain outside this evidence. Combined tests cover search/type/sort/folder composition, Trash restore, encrypted settings through folder staging and clean recovery, and lock retirement.
