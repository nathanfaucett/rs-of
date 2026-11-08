# Component Registry

## Vault Secrets (`src/lib/vault/secrets/components/`)

| Component | Purpose | Route |
|-----------|---------|-------|
| `SecretEditor.svelte` | Create/edit secret form (login, card, identity, sshKey, secureNote) | `/vault/new`, `/vault/edit/[id]` |
| `PasswordInput.svelte` | Password field with inline eye toggle + copy button | Used in `SecretEditor` |
| `GeneratorDialog.svelte` | Password/passphrase generator popup | Triggered from `SecretEditor` |
| `PasswordGenerator.svelte` | Generator settings + output | Inside `GeneratorDialog` |
| `LocalSecretEditor.svelte` | Vault list + mobile filter header + trash mode | `/vault`, `/trash` |
| `FolderControls.svelte` | Folder CRUD + move item dropdown | Inside `LocalSecretEditor` |
| `FilterSidebar.svelte` | Desktop filter sidebar (folders, types, favorites, sort) | `/vault` sidebar |
| `CardFields.svelte` | Credit card form fields | Inside `SecretEditor` |
| `IdentityFields.svelte` | Identity form fields | Inside `SecretEditor` |
| `SshKeyFields.svelte` | SSH key form fields | Inside `SecretEditor` |
| `ProtectedValue.svelte` | Masked value display with reveal/copy | Generator output |

## Layout (`src/routes/(app)/layout/`)

| Component | Purpose |
|-----------|---------|
| `NavSidebar.svelte` | Main navigation (Vault, Generator, Settings, Trash) |
| `NavSidebarItem.svelte` | Single nav item with icon/label |
| `FilterSidebar.svelte` | **Alias** - see Vault Secrets above |
| `FilterSidebarItem.svelte` | Unused (legacy) |

## Shared (`src/lib/shared/components/`)

| Component | Purpose |
|-----------|---------|
| `Dialog.svelte` | Accessible modal dialog with backdrop |

## UI (`src/lib/ui/`)

| Component | Purpose |
|-----------|---------|
| `Alert.svelte` | Toast/alert with tone (info/success/warning/danger) |
| `Card.svelte` | Section container with header/actions |
| `Field.svelte` | Form field wrapper with label/description/issues |
| `Page.svelte` | Page width container (wide/narrow) |
| `PageHeader.svelte` | Page title + description + actions |

## State (`src/lib/shared/state/`)

| Module | Purpose |
|--------|---------|
| `vault.svelte.ts` | Vault lifecycle (locked/unlocked/selector) + `vaultStateModule` |
| `notifications.svelte.ts` | Toast notifications |

## Workflows (`src/lib/vault/secrets/`)

| Module | Purpose |
|--------|---------|
| `workflow.svelte.ts` | `createLocalSecretWorkflow` - CRUD for secrets/folders |
| `filter-state.svelte.ts` | `createFilterState` - shared reactive filter state |
| `search.ts` | `searchItems` - client-side filtering/sorting |
| `secret-draft.ts` | Draft types + `createSecretDraft`/`constructSecret` |
