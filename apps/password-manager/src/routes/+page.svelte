<script lang="ts">
import { Eye, EyeOff } from "@lucide/svelte";
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

onMount(() => {
    void vaultStateModule.init();
});

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
    const synced = vaultStateModule.state.status === "create-vault" && vaultStateModule.state.synced;
    try {
        await vaultStateModule.createVault(pwd, vaultName.trim(), synced);
        password = "";
        confirmPassword = "";
        vaultName = "";
    } catch (cause) {
        if (isRedirect(cause)) throw cause;
        const recovery = synced
            ? "Return to the selector to recover an interrupted registration before retrying. Do not clear browser data."
            : "Check browser storage and free space. Return to the selector to recover an interrupted registration before retrying. Do not clear browser data.";
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

function showCreateVault(synced = false) {
    error = "";
    password = "";
    confirmPassword = "";
    vaultName = "";
    vaultStateModule.state = { status: "create-vault", synced };
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
                    {:else if syncStateModule.supportsReplication()}
                        <button class="btn btn-secondary btn-md" type="button" disabled={busy} onclick={showSyncedVaults}>Show synced vaults</button>
                        <button class="btn btn-secondary btn-md" type="button" disabled={busy} onclick={() => showCreateVault(true)}>Create synced vault</button>
                    {:else}
                        <p>Vault replication is not available with separate IdP and Storage bases. You can still use local vaults.</p>
                    {/if}
                    <button class="btn btn-secondary btn-md" type="button" onclick={() => { notice = ""; vaultStateModule.state = { status: "restore-vault" }; }}>Restore encrypted backup</button>
                    {#if vaultStateModule.state.vaults.length > 0}
                        <div class="flex flex-col gap-2">
                            <p class="text-sm font-medium text-muted-foreground">Existing vaults</p>
                            {#each vaultStateModule.state.vaults as identity (`${identity.synced ? "synced" : "local"}:${identity.id}`)}
                                <button
                                    class="btn btn-secondary btn-md text-left"
                                    type="button"
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
                        onclick={() => showCreateVault()}
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
                description={vaultStateModule.state.synced ? "Your vault is encrypted with your master password before sync." : "Your passwords are stored locally, encrypted with your master password. No account or server needed."}
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
