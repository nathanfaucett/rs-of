# Password manager: collection-backed read workflow

This slice signs in, discovers/creates filesystem Storage Resources, and opens secrets using `@aicacia/storage-db-collection` plus the **official** `@tanstack/svelte-db` live-query binding. New secret, Save, and Delete are deliberately disabled until ticket 03 supplies collection mutation handlers. The legacy SecretStorage implementation and direct writes have been removed. Ticket 04 adds explicit refresh and complete-workflow qualification.

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

Use the provisioned Installation's User and existing OAuth flow. The app's OAuth Client ID is `password-manager-web`, registered for Application URI `password-manager.aicacia.com`; the reusable adapter has neither hard-coded. The existing client exchanges the current User's token at the selected IdP and uses a Storage-Audience Access Token. Socket opening currently requests both read and write authorization, even for this read-only UI. Catalog discovery is not a resource-access grant.

## Behavior

- Select a Storage Resource by **Filesystem ID**, not its potentially duplicate name. Labels display both. Create filesystem opens the returned ID; on a fresh filesystem, the missing `secrets` directory is correctly empty.
- The collection eagerly validates one JSON file per secret at `secrets/<Secret ID>.json` using `SecretSchema`. Storage listing names are full paths. Secret URI arrays, custom fields, and history remain part of the loaded model; this slice does not rewrite any record.
- Initial loading, empty, ready, malformed-record, and authorization failure are distinct. A failed or partially malformed load is never shown as an empty filesystem. Ordering is favorite-first, then name, through the live query, not a second array in component state.
- Selecting another filesystem destroys the old collection/query and clears editor selection. The same applies to an authenticated User/IdP change, route teardown, or Sign out. Cross-tab authentication storage changes hide the browser while the auth layout revalidates. Late responses cannot populate a replacement view. Keys deliberately remount the application view at these security/lifecycle boundaries.
- **Open filesystem** creates a fresh collection for the current selection, so a failed initial open can be retried. It is not in-place refresh/reconciliation. There is no polling, automatic reconnect, or change subscription.
- Connections and decrypted records live in memory only. No browser secret persistence, IndexedDB, localStorage secret cache, or offline mutation outbox is added. The existing OAuth/session preferences are separate from secret storage.

The browser sees secret contents. This example is not evidence of end-to-end encryption or a production-grade password vault. Server-side Node replication does not make this initial browser snapshot live across clients. Sequential reads are not an atomic multi-record server snapshot.

## Repeatable checks

```sh
pnpm --filter password-manager test
pnpm --filter password-manager exec node --test svelte.config.test.mjs
pnpm --filter password-manager build
pnpm --filter password-manager check
```

Run the build before the first check in a clean checkout to generate the existing Paraglide runtime. `project.inlang/settings.json` follows the other app's workspace convention and is required by the production plugin.

The mounted Svelte tests use the real official binding, collection adapter, and Storage client with controlled transports. They cover discovery/create/open, duplicate names, read states, switching, late responses, User changes, and sign-out. Sign-in tests qualify form submission and the configured-authority/error boundary, not a live OAuth server. Package tests cover transport cancellation, schema/path isolation, all-or-error loading, and real DB ordering. No controlled test establishes real Node replication.

The existing broad `lint`/`format` scripts scan generated `.svelte-kit`, build, and Paraglide output as well as source. For a focused source check, pass the changed source/config paths to `pnpm exec biome lint` / `biome format`; validate Svelte with `svelte-check` and the Svelte autofixer. Workspace `just crap` is Rust-only and requires sibling Rust workspaces at the paths expected by Cargo; it does not measure this TypeScript integration.

## Real-system qualification (manual; not yet performed)

1. Provision a Node using [operations](../../docs/operations.md), use its configured service bases, and sign in as an allowed User. Confirm discovery/creation and opening by ID; create two resources with the same name to verify selection is unambiguous.
2. Open a newly created filesystem: expect a ready empty state. Open a permitted filesystem containing valid `SecretSchema` JSON records under `secrets/`: expect favorite/name ordering and read-only details. Seed records using an existing authorized Storage client; this slice intentionally has no write bypass.
3. Switch resources while an open is slow, then sign out while reading. Verify previous details disappear, sockets close, and no late secrets become visible. Sign in as another User and verify a clean namespace.
4. Test denied/expired authorization, malformed JSON/schema, and a dropped connection. Verify an error, not empty/success. Reopen explicitly after correcting the condition.
5. Reopen after changing a record externally to obtain a new initial snapshot. Do not assume a push feed, automatic recovery, offline writes, or cross-Node replication from this result. Persistent CRUD, refresh races, and full reload persistence qualification belong to tickets 03/04.
