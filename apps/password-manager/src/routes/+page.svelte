<script lang="ts">
import { Eye, EyeOff } from "@lucide/svelte";
import { SyncError } from "@ofsdk/sync-sdk/browser";
import { isRedirect } from "@sveltejs/kit";
import { onMount } from "svelte";
import { resolve } from "$app/paths";
import { authState } from "$lib/auth/auth.svelte";
import { syncStateModule } from "$lib/shared/state/sync.svelte";
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
let uploaded = $state(false);
let restoreCandidate = $state.raw<{ session: NonNullable<typeof authState.session>; id: string }>();
let binding = $state.raw<{ session: NonNullable<typeof authState.session>; id: string }>();
const restoredResourceId = $derived(binding?.session === authState.session ? binding?.id : undefined);

let destinationHasHistory = $state(false);

function showDestination() {
    confirmedSession = authState.session;
    resourceId = "";
    uploadError = "";
    uploaded = false;
    destinationHasHistory = false;
}

async function upload() {
    const session = confirmedSession;
    if (!session || session !== authState.session || busy || uploaded || destinationHasHistory) return;
    uploadError = "";
    resourceId = "";
    busy = true;
    try {
        await syncStateModule.uploadLocalDatabase(session, (id) => { resourceId = id; });
        uploaded = true;
    } catch (cause) {
        if (cause instanceof SyncError && cause.code === "destination-has-history") {
            destinationHasHistory = true;
            uploadError = "This Resource already has history. The app cannot verify an earlier upload. No merge was attempted. Keep using your local vaults.";
        } else if (cause instanceof SyncError && cause.code === "auth" && resourceId) {
            uploadError = "Storage did not authorize the upload. Check Selection with the serving Device owner. Only that owner can set Selection, with read access to this Resource and the devices.select permission for this Application. Once authorized, retry the first upload. It stops if the Resource has history.";
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
    void readRestoreBinding();
});

async function readRestoreBinding() {
    await authState.initialize();
    const session = authState.session;
    if (!session) return;
    try {
        const id = await syncStateModule.restoredResource(session);
        if (session === authState.session && id) binding = { session, id };
    } catch (cause) {
        error = cause instanceof Error ? cause.message : "Could not read the local restore record.";
    }
}

async function findRestore() {
    const session = authState.session;
    if (busy || !session) return;
    error = "";
    notice = "";
    busy = true;
    try {
        const id = await syncStateModule.findRestoreResource(session);
        if (id) restoreCandidate = { session, id };
        else notice = "No Password Manager database is available to restore.";
    } catch (cause) {
        error = cause instanceof Error ? cause.message : "Could not find the remote database.";
    } finally {
        busy = false;
    }
}

async function restore() {
    const candidate = restoreCandidate;
    if (busy || !candidate || candidate.session !== authState.session) return;
    busy = true;
    error = "";
    try {
        await syncStateModule.restoreRemoteDatabase(candidate.session, candidate.id);
        await vaultStateModule.showVaultSelector();
        binding = candidate;
        restoreCandidate = undefined;
        notice = "Remote database restored. Unlock its vaults with their existing passwords.";
    } catch (cause) {
        error = cause instanceof Error ? cause.message : "Remote restore failed. Try again.";
    } finally {
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
            {#if error}<Alert tone="danger">{error}</Alert>{/if}
            <Card>
                <div class="flex flex-col gap-4">
                    {#if !authState.isAuthenticated}
                        <a class="btn btn-secondary btn-md" href={resolve("/login")}>Sign in to sync</a>
                    {:else}
                        {#if !restoredResourceId && vaultStateModule.state.vaults.some((vault) => !vault.synced)}
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
                                    {#if uploaded}
                                        <p role="status">First upload complete. Your vaults stay on this browser.</p>
                                    {:else if !destinationHasHistory}
                                        <button class="btn btn-primary btn-md" type="button" disabled={busy} onclick={upload}>
                                            {busy ? "Uploading…" : uploadError ? "Retry first upload" : "Confirm and upload"}
                                        </button>
                                    {/if}
                                    <button class="btn btn-secondary btn-md" type="button" disabled={busy} onclick={() => confirmedSession = null}>
                                        {uploaded ? "Done" : "Cancel"}
                                    </button>
                                </section>
                            {:else}
                                <button class="btn btn-secondary btn-md" type="button" disabled={busy} onclick={showDestination}>Connect local database</button>
                            {/if}
                        {/if}
                        {#if restoredResourceId}
                            <p>Restored Storage Resource ID: {restoredResourceId}</p>
                        {:else if restoreCandidate && restoreCandidate.session === authState.session}
                            <section class="flex flex-col gap-2" aria-labelledby="restore-source">
                                <h2 id="restore-source" class="font-medium">Confirm remote database restore</h2>
                                <p class="break-all">Restore from {restoreCandidate.session.storage}. Storage Resource ID: {restoreCandidate.id}</p>
                                <p>The restore only downloads. It stops if this browser has local history.</p>
                                <button class="btn btn-primary btn-md" type="button" disabled={busy} onclick={restore}>Restore database</button>
                                <button class="btn btn-secondary btn-md" type="button" disabled={busy} onclick={() => restoreCandidate = undefined}>Cancel restore</button>
                            </section>
                        {:else}
                            <button class="btn btn-secondary btn-md" type="button" disabled={busy} onclick={findRestore}>Find remote database</button>
                        {/if}
                        {#if syncStateModule.supportsReplication()}
                            <button class="btn btn-secondary btn-md" type="button" disabled={busy} onclick={showSyncedVaults}>Show synced vaults</button>
                        {/if}
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
