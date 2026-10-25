# Password manager: collection-backed secrets

Sign in, discover/create filesystem Storage Resources, and create, edit, or delete secrets using `@aicacia/storage-db-collection` plus the **official** `@tanstack/svelte-db` live-query binding. The legacy SecretStorage implementation and direct writes have been removed; only form state remains local. Ticket 04 adds explicit refresh and complete-workflow qualification.

## Local setup

From the workspace root:

```sh
pnpm install --frozen-lockfile
pnpm --filter @aicacia/idp-client build:esm
pnpm --filter @aicacia/native-fetch build:esm
pnpm --filter @aicacia/oidc-client build:esm
pnpm --filter @aicacia/storage-client build:esm
pnpm --filter @aicacia/storage-db-collection build
pnpm --filter @aicacia/svelte-headless prepack
```

Copy this app's `.env.example` to `.env` and configure `PUBLIC_URL`, `PUBLIC_IDP_BASE_URL`, and `PUBLIC_STORAGE_BASE_URL` for your running, provisioned Installation. Keep explicit service prefixes (for example `/idp` and `/storage`); Storage need not share the IdP host. `PUBLIC_LIDP_BASE_URL` is obsolete and is no longer read. The sign-in screen can select a different IdP service base; an explicitly selected base is retained by the existing `idp-api-url` preference. Configure Storage independently. Changing an environment default does not overwrite that preference.

```sh
pnpm --filter password-manager dev
```

Use the provisioned Installation's User and existing OAuth flow. The app's OAuth Client ID is `password-manager-web`, registered for Application URI `password-manager.aicacia.com`; the reusable adapter has neither hard-coded. The existing client exchanges the current User's token at the selected IdP and uses a Storage-Audience Access Token. Socket opening requests both read and write authorization. Catalog discovery is not a resource-access grant.

## Behavior

- Select a Storage Resource by **Filesystem ID**, not its potentially duplicate name. Labels display both. Create filesystem opens the returned ID; on a fresh filesystem, the missing `secrets` directory is correctly empty.
- The collection eagerly validates one JSON file per secret at `secrets/<Secret ID>.json` using `SecretSchema`. Storage listing names are full paths. The editor preserves additional URIs, the first URI's match mode, custom fields, creation/deletion timestamps, unedited notes, and existing history. Clearing Website removes only the first URI. Changing Secret appends the previous secret and a change timestamp; other edits do not add history. Save validates required Name/Secret and updates `updatedAt`.
- Initial loading, empty, ready, malformed-record, and authorization failure are distinct. A failed or partially malformed load is never shown as an empty filesystem. Ordering is favorite-first, then name, through the live query, not a second array in component state.
- Selecting another filesystem destroys the old collection/query and clears editor selection. The same applies to an authenticated User/IdP change, route teardown, or Sign out. Cross-tab authentication storage changes hide the browser while the auth layout revalidates. Late responses cannot populate a replacement view. Keys deliberately remount the application view at these security/lifecycle boundaries.
- New secret, Save, and Delete use collection mutations. The list responds optimistically; controls are disabled while persistence and authoritative reload are pending. Success appears only after settlement. Reopen or reload to verify persistence. There is no separate secrets array or direct Storage write bypass.
- A rejected write rolls the list back to reconciled authoritative records and keeps form edits visible, with an error rather than a success notice. Review before retrying. An unacknowledged write can have persisted: after successful reconciliation the error remains visible and the save editor closes so it cannot blindly create a duplicate. Select the resulting record to inspect it.
- Disconnect or failed reconciliation hides the list/editor and warns that a sent write may have persisted. **Open filesystem** and inspect the outcome before retrying. There are no automatic write retries or durable offline writes. A silent still-connected endpoint can leave a mutation pending; switching/reopening disposes that view but cannot cancel a write already sent.
- Switching filesystem or authenticated context during a write clears old records and drafts; old completion/error notices cannot appear as the new context's result. A sent write may finish in the old filesystem.
- **Open filesystem** creates a fresh collection for the current selection, so a failed initial open can be retried. It is not in-place refresh/reconciliation. There is no polling, automatic reconnect, or change subscription.
- Connections and decrypted records live in memory only. No browser secret persistence, IndexedDB, localStorage secret cache, or offline mutation outbox is added. The existing OAuth/session preferences are separate from secret storage.

The browser sees secret contents. This example is not evidence of end-to-end encryption or a production-grade password vault. Server-side Node replication does not make the browser snapshot live across clients. Sequential reads are not an atomic multi-record server snapshot. The adapter supports single-record mutations only and rejects batches before writing. It does not provide compare-and-swap or protect against another client's concurrent edits.

## Repeatable checks

```sh
pnpm --filter password-manager test
pnpm --filter password-manager exec node --test svelte.config.test.mjs
pnpm --filter password-manager build
pnpm --filter password-manager check
```

Run the build before the first check in a clean checkout to generate the existing Paraglide runtime. `project.inlang/settings.json` follows the other app's workspace convention and is required by the production plugin.

The mounted Svelte tests use the real official binding, collection adapter, and Storage client with controlled transports. They cover discovery/create/open, duplicate names, CRUD/reopen persistence, unedited-field/history preservation, validation, rollback, ambiguous outcomes, and filesystem/User/sign-out isolation during writes. Sign-in tests qualify form submission and the configured-authority/error boundary, not a live OAuth server. Package tests cover transport cancellation, schema/path isolation, all-or-error loading, real DB ordering, CRUD settlement against authoritative snapshots, pending-optimistic isolation, rejected/ambiguous writes, and disposal during validation/writes. No controlled test establishes real Node replication.

The existing broad `lint`/`format` scripts scan generated `.svelte-kit`, build, and Paraglide output as well as source. For a focused source check, pass the changed source/config paths to `pnpm exec biome lint` / `biome format`; validate Svelte with `svelte-check` and the Svelte autofixer. Workspace `just crap` is Rust-only and requires sibling Rust workspaces at the paths expected by Cargo; it does not measure this TypeScript integration.

## Real-system qualification (manual; not yet performed)

1. Provision a Node using [operations](../../docs/operations.md), use its configured service bases, and sign in as an allowed User. Confirm discovery/creation and opening by ID; create two resources with the same name to verify selection is unambiguous.
2. Open a newly created filesystem: expect a ready empty state. Create a secret, edit it, change its password, and delete another secret. Reopen and reload the app after each operation to verify persistence. With authorized seeded records containing multiple URIs, custom fields, and history, verify edits preserve unedited data and append history only when the password changes. Check favorite/name ordering.
3. Switch resources while an open or write is slow, then sign out during a write. Verify previous details disappear, sockets close, and no late secrets become visible. Sign in as another User and verify a clean namespace.
4. Test denied/expired authorization, malformed JSON/schema, and a dropped connection. Verify an error, not empty/success. Reopen explicitly after correcting the condition.
5. Reopen after changing a record externally to obtain a new initial snapshot. Do not assume a push feed, automatic recovery, offline writes, or cross-Node replication from this result. Controlled CRUD/reopen checks do not replace this deployed-system exercise. Explicit refresh races and full-workflow qualification remain ticket 04.
