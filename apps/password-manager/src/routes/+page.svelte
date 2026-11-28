<script lang="ts">
import { Eye, EyeOff } from "@lucide/svelte";
import { SyncError } from "@ofsdk/sync-sdk/browser";
import { isRedirect } from "@sveltejs/kit";
import { onMount } from "svelte";
import { resolve } from "$app/paths";
import { authState } from "$lib/auth/auth.svelte";
import { BindingSaveError, syncStateModule } from "$lib/shared/state/sync.svelte";
import { vaultStateModule } from "$lib/shared/state/vault.svelte";
import { Alert, Card, Field, Page, PageHeader } from "$lib/ui";
import RestoreBackup from "$lib/vault/RestoreBackup.svelte";
import type { VaultIdentity } from "$lib/vault/vault";

const minimumPasswordLength = 8;

let password = $state("");
let confirmPassword = $state("");
let showPassword = $state(false);
let vaultName = $state("");
let error = $state("");
let busy = $state(false);
let notice = $state("");
let confirmedSession = $state.raw<typeof authState.session>(null);
let resourceId = $state("");
let uploadError = $state("");
type Session = NonNullable<typeof authState.session>;
type RestoreInspection = NonNullable<Awaited<ReturnType<typeof syncStateModule.inspectRestore>>>;
let restoreCandidate = $state.raw<RestoreInspection & { session: Session }>();
const currentCandidate = $derived(restoreCandidate?.session === authState.session ? restoreCandidate : undefined);
let binding = $state.raw<{ session: Session; value: Awaited<ReturnType<typeof syncStateModule.getBinding>> }>();
const currentBinding = $derived(binding?.session === authState.session ? binding?.value : undefined);
const syncStatus = $derived(syncStateModule.getSyncStatus(
    authState.session,
    currentBinding?.status === "bound" ? currentBinding.id : undefined
));
let signingOut = $state(false);

let uploadBlocked = $state(false);

function showDestination() {
    confirmedSession = authState.session;
    resourceId = "";
    uploadError = "";
    uploadBlocked = false;
}

async function upload() {
    const session = confirmedSession;
    if (!session || session !== authState.session || busy || uploadBlocked) return;
    uploadError = "";
    resourceId = "";
    busy = true;
    try {
        await syncStateModule.uploadLocalDatabase(session, (id) => { resourceId = id; });
        await readBinding(session);
        notice = "First upload complete. Your vaults stay on this browser.";
    } catch (cause) {
        if (cause instanceof SyncError && cause.code === "destination-has-history") {
            uploadBlocked = true;
            uploadError = "This Resource already has history. The app cannot verify an earlier upload. No merge was attempted. Keep using your local vaults.";
        } else if (cause instanceof SyncError && cause.code === "auth" && resourceId) {
            uploadError = "Storage did not authorize the upload. Check Selection with the serving Device owner. Only that owner can set Selection, with read access to this Resource and the devices.select permission for this Application. Once authorized, retry the first upload. It stops if the Resource has history.";
        } else if (cause instanceof BindingSaveError) {
            error = cause.message;
            await readBinding(session);
        } else if (resourceId) {
            uploadError = "Storage did not confirm publication. It may have saved the upload. You can keep using your local vaults. A retry uses the same Resource and stops if it has history.";
        } else {
            uploadError = cause instanceof Error ? cause.message : "First upload failed. Try again.";
        }
    } finally {
        busy = false;
    }
}

onMount(() => {
    void vaultStateModule.init();
    void authState.initialize().then(() => readBinding(authState.session));
});

async function readBinding(session: typeof authState.session) {
    if (!session) return;
    try {
        const value = await syncStateModule.getBinding(session);
        if (session === authState.session) binding = { session, value };
    } catch (cause) {
        error = cause instanceof Error ? cause.message : "Could not read the local Storage binding.";
    }
}

async function retryLocalSave() {
    const session = authState.session;
    if (!session || busy) return;
    busy = true;
    error = "";
    try {
        await syncStateModule.retryBindingSave(session);
        await readBinding(session);
        if (session === authState.session) notice = "Storage binding saved. Sync is detached. Your vaults stay on this browser.";
    } catch (cause) {
        error = cause instanceof Error ? cause.message : "Could not save the Storage binding.";
        await readBinding(session);
    } finally {
        busy = false;
    }
}

async function syncNow() {
    const session = authState.session;
    if (!session || busy || !syncStatus) return;
    error = "";
    notice = "";
    try {
        await syncStateModule.syncBoundDatabase(session);
    } catch {
        // The SDK reports exchange failures. Detach and sign-out clear its status.
    }
}

async function signOut() {
    const session = authState.session;
    if ((!session && !authState.logoutError) || signingOut) return;
    signingOut = true;
    error = "";
    try {
        if (session) await syncStateModule.signOut(session);
        else await authState.logout();
    } catch (cause) {
        error = cause instanceof Error ? cause.message : "Could not detach Storage and sign out.";
    } finally {
        signingOut = false;
    }
}

async function changeDetachment(detached: boolean) {
    const session = authState.session;
    if (!session || busy || currentBinding?.status !== "bound") return;
    busy = true;
    error = "";
    notice = "";
    try {
        await syncStateModule.setDetached(session, currentBinding.id, detached);
    } catch (cause) {
        error = cause instanceof Error ? cause.message : "Could not change Storage attachment.";
        await readBinding(session);
    } finally {
        busy = false;
    }
}

async function findRestore() {
    const session = authState.session;
    if (busy || !session) return;
    error = "";
    notice = "";
    busy = true;
    restoreCandidate = undefined;
    try {
        const candidate = await syncStateModule.inspectRestore(session);
        if (candidate) restoreCandidate = { session, ...candidate };
        else notice = "No Password Manager database is available to restore.";
    } catch (cause) {
        error = cause instanceof Error ? cause.message : "Could not find the remote database.";
    } finally {
        busy = false;
    }
}

async function restore() {
    const candidate = restoreCandidate;
    if (busy || candidate?.mode !== "restore" || candidate.session !== authState.session) return;
    busy = true;
    error = "";
    try {
        await syncStateModule.restoreRemoteDatabase(candidate.session, candidate.id);
        await vaultStateModule.showVaultSelector();
        await readBinding(candidate.session);
        restoreCandidate = undefined;
        notice = "Remote database restored. Unlock its vaults with their existing passwords.";
    } catch (cause) {
        error = cause instanceof Error ? cause.message : "Remote restore failed. Try again.";
    } finally {
        busy = false;
    }
}

async function merge(retry = false) {
    const session = authState.session;
    const candidate = currentCandidate;
    if (!session || busy || (retry ? currentBinding?.status !== "pending-merge" || !currentBinding.canRetry : candidate?.mode !== "merge")) return;
    busy = true;
    error = "";
    notice = "";
    restoreCandidate = undefined;
    confirmedSession = null;
    try {
        await syncStateModule.mergeDatabases(session, retry || candidate?.mode !== "merge" ? undefined : candidate);
        await vaultStateModule.showVaultSelector();
        if (session === authState.session) notice = "Databases merged. Unlock each vault with its existing password. Sync is detached.";
    } catch (cause) {
        if (session === authState.session) error = cause instanceof Error ? cause.message : "Merge did not complete.";
    } finally {
        await readBinding(session);
        busy = false;
    }
}

async function resolvePendingMerge() {
    const session = authState.session;
    if (!session || busy || currentBinding?.status !== "pending-merge") return;
    busy = true;
    error = "";
    notice = "";
    try {
        const committed = await syncStateModule.resolvePendingMerge(session);
        if (session === authState.session) {
            notice = `${committed ? "Storage already committed the old merge." : "Storage cancelled the old merge."} Your local data and the old candidate were kept. Find the remote database and approve a new merge if you want to continue.`;
        }
    } catch (cause) {
        if (session === authState.session) error = cause instanceof Error ? cause.message : "Could not check the pending merge.";
    } finally {
        await readBinding(session);
        busy = false;
    }
}

async function createVault() {
    if (busy) return;
    error = "";
    if (!vaultName.trim()) {
        error = "Vault name is required.";
        return;
    }
    if (!password) {
        error = "Password is required.";
        return;
    }
    if (password !== confirmPassword) {
        error = "Passwords do not match.";
        return;
    }
    if (password.length < minimumPasswordLength) {
        error = `Password must be at least ${minimumPasswordLength} characters.`;
        return;
    }
    busy = true;
    const pwd = password;
    try {
        await vaultStateModule.createVault(pwd, vaultName.trim());
        password = "";
        confirmPassword = "";
        vaultName = "";
    } catch (cause) {
        if (isRedirect(cause)) throw cause;
        const recovery = "Check browser storage and free space. Return to the selector to recover an interrupted registration before retrying. Do not clear browser data.";
        error = `${cause instanceof Error ? cause.message : "Failed to create vault."} ${recovery}`;
    } finally {
        busy = false;
    }
}

async function unlockVault() {
    error = "";
    if (busy || vaultStateModule.state.status !== "unlock-vault") return;
    if (!password) {
        error = "Password is required.";
        return;
    }
    busy = true;
    const pwd = password;
    try {
        await vaultStateModule.unlockExistingVault(pwd, vaultStateModule.state.identity);
        password = "";
    } catch (cause) {
        if (isRedirect(cause)) throw cause;
        error = cause instanceof Error ? cause.message : "Failed to unlock vault. Wrong password?";
    } finally {
        busy = false;
    }
}

function selectVault(identity: VaultIdentity & { synced?: boolean }) {
    showPassword = false;
    notice = "";
    error = "";
    password = "";
    vaultStateModule.state = { status: "unlock-vault", identity };
}

function showCreateVault() {
    error = "";
    password = "";
    confirmPassword = "";
    vaultName = "";
    vaultStateModule.state = { status: "create-vault" };
}

async function showSyncedVaults() {
    if (busy) return;
    error = "";
    notice = "";
    busy = true;
    try {
        if (await vaultStateModule.showSyncedVaults() === 0) notice = "No synced vaults.";
    } catch (cause) {
        error = cause instanceof Error ? cause.message : "Failed to list synced vaults.";
    } finally {
        busy = false;
    }
}

async function restored(identity: { id: string; name: string }) {
    await vaultStateModule.showVaultSelector();
    notice = `Restore verified: ${identity.name}. ID: ${identity.id}. Unlock it and check the data before discarding the backup.`;
}
const title = $derived(vaultStateModule.state.status === "create-vault" ? "Create vault | Password Manager"
    : vaultStateModule.state.status === "unlock-vault" ? "Unlock vault | Password Manager"
    : vaultStateModule.state.status === "restore-vault" ? "Restore vault | Password Manager"
    : "Password Manager");
</script>

<svelte:head><title>{title}</title></svelte:head>

<div class="flex min-h-0 grow items-start justify-center overflow-auto p-6">
    {#if vaultStateModule.state.status === "loading"}
        <Page width="narrow">
            <p role="status">Loading…</p>
        </Page>
    {:else if vaultStateModule.state.status === "select-vault"}
        <Page width="narrow">
            <PageHeader
                title="Password Manager"
                description="Select a vault to unlock, or create a new one."
            />
            {#if notice}<p role="status">{notice}</p>{/if}
            {#if authState.logoutError}<Alert tone="danger">{authState.logoutError}</Alert>
            {:else if error}<Alert tone="danger">{error}</Alert>{/if}
            <Card>
                <div class="flex flex-col gap-4">
                    {#if authState.logoutError}
                        <button class="btn btn-secondary btn-md" type="button" disabled={signingOut} onclick={signOut}>Retry sign-out</button>
                    {:else if !authState.isAuthenticated}
                        <a class="btn btn-secondary btn-md" href={resolve("/login")}>Sign in to sync</a>
                    {:else}
                        {#if currentBinding?.status === "unbound" && !currentCandidate && vaultStateModule.state.vaults.some((vault) => !vault.synced)}
                            {#if confirmedSession && confirmedSession === authState.session}
                                <section class="flex flex-col gap-2" aria-labelledby="sync-destination">
                                    <h2 id="sync-destination" class="font-medium">Confirm sync destination</h2>
                                    <p>Upload all local vaults to this destination. Secret records are encrypted. Vault names and other metadata are not.</p>
                                    <dl class="break-all">
                                        <dt class="font-medium">Installation (issuer)</dt><dd>{confirmedSession.issuer}</dd>
                                        <dt class="font-medium">User</dt>
                                        <dd>{confirmedSession.userEmail ? `${confirmedSession.userEmail} (${confirmedSession.user.sub})` : confirmedSession.user.sub}</dd>
                                        <dt class="font-medium">Application</dt><dd>{confirmedSession.applicationUri}</dd>
                                        <dt class="font-medium">Storage</dt><dd>{confirmedSession.storage}</dd>
                                        {#if resourceId}
                                            <dt class="font-medium">Storage Resource ID</dt><dd>{resourceId}</dd>
                                        {/if}
                                    </dl>
                                    {#if uploadError}<Alert tone="danger">{uploadError}</Alert>{/if}
                                    {#if !uploadBlocked}
                                        <button class="btn btn-primary btn-md" type="button" disabled={busy} onclick={upload}>
                                            {busy ? "Uploading…" : uploadError ? "Retry first upload" : "Confirm and upload"}
                                        </button>
                                    {/if}
                                    <button class="btn btn-secondary btn-md" type="button" disabled={busy} onclick={() => confirmedSession = null}>
                                        Cancel
                                    </button>
                                </section>
                            {:else}
                                <button class="btn btn-secondary btn-md" type="button" disabled={busy} onclick={showDestination}>Connect local database</button>
                            {/if}
                        {/if}
                        {#if currentBinding?.status === "bound"}
                            <p class="break-all">Linked Storage Resource ID: {currentBinding.id}</p>
                            {#if !syncStatus}
                                <p>Sync is detached in this tab. Reattach to send and receive changes automatically.</p>
                                <button class="btn btn-secondary btn-md" type="button" disabled={busy} onclick={() => changeDetachment(false)}>Reattach sync</button>
                            {:else}
                                <button class="btn btn-primary btn-md" type="button" disabled={busy || syncStatus.status === "syncing"} onclick={syncNow}>Sync now</button>
                                <button class="btn btn-secondary btn-md" type="button" disabled={busy} onclick={() => changeDetachment(true)}>Detach sync</button>
                                {#if syncStatus.status === "error"}
                                    <Alert tone="danger">Sync failed. Check your connection, Storage access, and Selection. Your changes stay local. Retry with Sync now.</Alert>
                                {:else}
                                    <p role="status">{syncStatus.status === "syncing" ? "Syncing…" : "Sync complete."}</p>
                                {/if}
                            {/if}
                        {:else if currentBinding?.status === "pending-save"}
                            <p class="break-all">Storage confirmed upload to Resource ID: {currentBinding.id}</p>
                            <button class="btn btn-primary btn-md" type="button" disabled={busy} onclick={retryLocalSave}>Retry local save</button>
                        {:else if currentBinding?.status === "pending-merge"}
                            <p class="break-all">Storage Resource ID: {currentBinding.id}</p>
                            <p>{currentBinding.committed ? "Storage confirmed the merge. Local publication is not complete." : "Storage has not confirmed the merge. Check its receipt before retrying."}</p>
                            {#if currentBinding.canRetry}
                                <button class="btn btn-primary btn-md" type="button" disabled={busy} onclick={() => merge(true)}>Retry pending merge</button>
                            {:else}
                                <p>This sign-in cannot retry the old merge. You can check its outcome and archive it.</p>
                            {/if}
                            <button class="btn btn-secondary btn-md" type="button" disabled={busy} onclick={resolvePendingMerge}>Cancel or archive pending merge</button>
                        {:else if currentBinding?.status === "merge-different-account"}
                            <p>A merge is pending for a different account or Installation. Your local vaults are still available.</p>
                        {:else if currentBinding?.status === "different-account"}
                            <p>This database is linked to another account or Installation. Sign in to that account to sync. You can still use your local vaults.</p>
                        {:else if currentBinding?.status === "unbound" && currentCandidate}
                            <section class="flex flex-col gap-2" aria-labelledby="restore-source">
                                <h2 id="restore-source" class="font-medium">{currentCandidate.mode === "merge" ? "Confirm database merge" : currentCandidate.mode === "restore" ? "Confirm remote database restore" : "Remote database cannot be restored here"}</h2>
                                {#if currentCandidate.mode === "merge"}
                                    <p>Merge the entire local and remote databases, including all vaults and deletion history. Secret records are encrypted. Vault names and other metadata are not. Approval sends the merged history to this destination.</p>
                                    <dl class="break-all">
                                        <dt class="font-medium">Installation (issuer)</dt><dd>{currentCandidate.session.issuer}</dd>
                                        <dt class="font-medium">User</dt><dd>{currentCandidate.session.user.sub}</dd>
                                        <dt class="font-medium">Application</dt><dd>{currentCandidate.session.applicationUri}</dd>
                                        <dt class="font-medium">Storage</dt><dd>{currentCandidate.session.storage}</dd>
                                        <dt class="font-medium">Storage Resource ID</dt><dd>{currentCandidate.id}</dd>
                                    </dl>
                                    <button class="btn btn-primary btn-md" type="button" disabled={busy} onclick={() => merge()}>Approve and merge</button>
                                {:else}
                                    <p class="break-all">Storage: {currentCandidate.session.storage}. Storage Resource ID: {currentCandidate.id}</p>
                                    {#if currentCandidate.mode === "restore"}
                                        <p>The restore only downloads. It stops if this browser has local history.</p>
                                        <button class="btn btn-primary btn-md" type="button" disabled={busy} onclick={restore}>Restore database</button>
                                    {:else}
                                        <p>This browser has legacy local vaults. Both databases stay unchanged.</p>
                                    {/if}
                                {/if}
                                <button class="btn btn-secondary btn-md" type="button" disabled={busy} onclick={() => restoreCandidate = undefined}>{currentCandidate.mode === "merge" ? "Decline merge" : "Cancel restore"}</button>
                            </section>
                        {:else if currentBinding?.status === "unbound"}
                            <button class="btn btn-secondary btn-md" type="button" disabled={busy} onclick={findRestore}>Find remote database</button>
                        {/if}
                        {#if syncStateModule.supportsReplication()}
                            <button class="btn btn-secondary btn-md" type="button" disabled={busy} onclick={showSyncedVaults}>Show synced vaults</button>
                        {/if}
                        <button class="btn btn-secondary btn-md" type="button" disabled={signingOut} onclick={signOut}>Sign out</button>
                    {/if}
                    <button class="btn btn-secondary btn-md" type="button" disabled={busy} onclick={() => { notice = ""; vaultStateModule.state = { status: "restore-vault" }; }}>Restore encrypted backup</button>
                    {#if vaultStateModule.state.vaults.length > 0}
                        <div class="flex flex-col gap-2">
                            <p class="text-sm font-medium text-muted-foreground">Existing vaults</p>
                            {#each vaultStateModule.state.vaults as identity (`${identity.synced ? "synced" : "local"}:${identity.id}`)}
                                <button
                                    class="btn btn-secondary btn-md text-left"
                                    type="button"
                                    disabled={busy}
                                    onclick={() => selectVault(identity)}
                                >
                                    <span class="vault-name">{identity.name}{identity.synced ? " (synced)" : ""}</span>
                                    <span class="block break-all text-xs">ID: {identity.id}</span>
                                </button>
                            {/each}
                        </div>
                        <hr class="my-2" />
                    {/if}
                    <button
                        class="btn btn-primary btn-md"
                        type="button"
                        disabled={busy}
                        onclick={showCreateVault}
                    >
                        Create new vault
                    </button>
                </div>
            </Card>
        </Page>
    {:else if vaultStateModule.state.status === "restore-vault"}
        <Page width="narrow">
            <RestoreBackup onrestored={restored} oncancel={() => void vaultStateModule.showVaultSelector()} />
        </Page>
    {:else if vaultStateModule.state.status === "create-vault"}
        <Page width="narrow">
            <PageHeader
                title="Create your vault"
                description="Your passwords are stored locally, encrypted with your master password. No account or server needed."
            />
            <Card>
                <form class="flex flex-col gap-4" onsubmit={(event) => { event.preventDefault(); void createVault(); }}>
                    {#if error}
                        <Alert tone="danger">{error}</Alert>
                    {/if}
                    <Field id="vault-name" label="Vault name" required>
                        {#snippet children(control)}
                            <input
                                {...control}
                                {@attach (element) => element.focus()}
                                bind:value={vaultName}
                                type="text"
                                placeholder="e.g., Personal, Work, Family"
                                disabled={busy}
                            />
                        {/snippet}
                    </Field>
                    <Field id="password" label="Master password" description={`Use at least ${minimumPasswordLength} characters.`} required>
                        {#snippet children(control)}
                            <input
                                {...control}
                                bind:value={password}
                                type="password"
                                autocomplete="new-password"
                                disabled={busy}
                            />
                        {/snippet}
                    </Field>
                    <Field id="confirm" label="Confirm password" required>
                        {#snippet children(control)}
                            <input
                                {...control}
                                bind:value={confirmPassword}
                                type="password"
                                autocomplete="new-password"
                                disabled={busy}
                            />
                        {/snippet}
                    </Field>
                    <div class="flex gap-2">
                        <button
                            class="btn btn-secondary btn-md"
                            type="button"
                            disabled={busy}
                            onclick={vaultStateModule.showVaultSelector}
                        >
                            Cancel
                        </button>
                        <button
                            class="btn btn-primary btn-md"
                            type="submit"
                            disabled={busy}
                        >
                            {busy ? "Creating…" : "Create vault"}
                        </button>
                    </div>
                </form>
            </Card>
        </Page>
    {:else if vaultStateModule.state.status === "unlock-vault"}
        <Page width="narrow">
            <PageHeader
                title="Unlock vault: {vaultStateModule.state.identity.name}"
                description="Vault ID: {vaultStateModule.state.identity.id}. Enter your master password to access your passwords."
            />
            <Card>
                <form class="flex flex-col gap-4" onsubmit={(event) => { event.preventDefault(); void unlockVault(); }}>
                    {#if error}
                        <Alert tone="danger">{error}</Alert>
                    {/if}
                    <Field id="password" label="Master password" required>
                        {#snippet children(control)}
                            <div class="flex gap-2">
                                <input
                                    {...control}
                                    {@attach (element) => element.focus()}
                                    bind:value={password}
                                    type={showPassword ? "text" : "password"}
                                    disabled={busy}
                                    autocomplete="current-password"
                                    class="flex-1"
                                />
                                <button
                                    type="button"
                                    class="btn btn-ghost btn-md self-end"
                                    onclick={() => showPassword = !showPassword}
                                    aria-label={showPassword ? "Hide master password" : "Show master password"}
                                    aria-pressed={showPassword}
                                    disabled={busy}
                                >
                                    {#if showPassword}
                                        <EyeOff class="w-5 h-5" />
                                    {:else}
                                        <Eye class="w-5 h-5" />
                                    {/if}
                                </button>
                            </div>
                        {/snippet}
                    </Field>
                    <div class="flex gap-2">
                        <button
                            class="btn btn-secondary btn-md"
                            type="button"
                            disabled={busy}
                            onclick={vaultStateModule.showVaultSelector}
                        >
                            Back
                        </button>
                        <button
                            class="btn btn-primary btn-md"
                            type="submit"
                            disabled={busy}
                        >
                            {busy ? "Unlocking…" : "Unlock"}
                        </button>
                    </div>
                </form>
            </Card>
        </Page>
    {:else if vaultStateModule.state.status === "error"}
        <Page width="narrow">
            <Alert tone="danger" title="Error">{vaultStateModule.state.message}</Alert>
            <div class="mt-4">
                <button type="button" class="btn btn-secondary btn-md" onclick={vaultStateModule.showVaultSelector}>Try again</button>
            </div>
        </Page>
    {/if}
</div>
