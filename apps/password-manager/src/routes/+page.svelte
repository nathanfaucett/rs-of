<script lang="ts">
import { onMount } from "svelte";
import { isRedirect } from "@sveltejs/kit";
import RestoreBackup from "$lib/vault/RestoreBackup.svelte";
import { Alert, Card, Field, Page, PageHeader } from "$lib/ui";
import { Eye, EyeOff } from "@lucide/svelte";
import { vaultStateModule } from "$lib/shared/state/vault.svelte";

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
    if (password.length < 8) {
        error = "Password must be at least 8 characters.";
        return;
    }
    if (busy) return;
    busy = true;
    const pwd = password;
    try {
        await vaultStateModule.createVault(pwd, vaultName.trim());
        password = "";
        confirmPassword = "";
        vaultName = "";
    } catch (cause) {
        if (isRedirect(cause)) throw cause;
        error = `${cause instanceof Error ? cause.message : "Failed to create vault."} Check browser storage and free space. Return to the selector to recover an interrupted registration before retrying. Do not clear browser data.`;
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
        await vaultStateModule.unlockExistingVault(pwd, vaultStateModule.state.identity.id);
        password = "";
    } catch (cause) {
        if (isRedirect(cause)) throw cause;
        error = cause instanceof Error ? cause.message : "Failed to unlock vault. Wrong password?";
    } finally {
        busy = false;
    }
}

function selectVault(identity: { id: string; name: string }) {
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

async function restored(identity: { id: string; name: string }) {
    await vaultStateModule.showVaultSelector();
    notice = `Restore verified: ${identity.name}. ID: ${identity.id}. Unlock it and check the data before discarding the backup.`;
}
</script>

<svelte:window onfocus={() => vaultStateModule.state.status === "unlocked" && vaultStateModule.vaultData?.inactivity.check()} onpageshow={() => vaultStateModule.state.status === "unlocked" && vaultStateModule.vaultData?.inactivity.check()} />
<svelte:document onvisibilitychange={() => vaultStateModule.state.status === "unlocked" && vaultStateModule.vaultData?.inactivity.check()} />

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
            <Card>
                <div class="flex flex-col gap-4">
                    <button class="btn btn-secondary btn-md" type="button" onclick={() => { notice = ""; vaultStateModule.state = { status: "restore-vault" }; }}>Restore encrypted backup</button>
                    {#if vaultStateModule.state.vaults.length > 0}
                        <div class="flex flex-col gap-2">
                            <p class="text-sm font-medium text-muted-foreground">Existing vaults</p>
                            {#each vaultStateModule.state.vaults as identity (identity.id)}
                                <button
                                    class="btn btn-secondary btn-md text-left"
                                    type="button"
                                    onclick={() => selectVault(identity)}
                                >
                                    <span class="vault-name">{identity.name}</span>
                                    <span class="block break-all text-xs">ID: {identity.id}</span>
                                </button>
                            {/each}
                        </div>
                        <hr class="my-2" />
                    {/if}
                    <button
                        class="btn btn-primary btn-md"
                        type="button"
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
                <div class="flex flex-col gap-4">
                    {#if error}
                        <Alert tone="danger">{error}</Alert>
                    {/if}
                    <Field id="vault-name" label="Vault name" required>
                        {#snippet children(control)}
                            <input
                                {...control}
                                bind:value={vaultName}
                                type="text"
                                placeholder="e.g., Personal, Work, Family"
                                disabled={busy}
                            />
                        {/snippet}
                    </Field>
                    <Field id="password" label="Master password" required>
                        {#snippet children(control)}
                            <input
                                {...control}
                                bind:value={password}
                                type="password"
                                placeholder="At least 8 characters"
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
                            type="button"
                            disabled={busy}
                            onclick={() => void createVault()}
                        >
                            {busy ? "Creating…" : "Create vault"}
                        </button>
                    </div>
                </div>
            </Card>
        </Page>
    {:else if vaultStateModule.state.status === "unlock-vault"}
        <Page width="narrow">
            <PageHeader
                title="Unlock vault: {vaultStateModule.state.identity.name}"
                description="Vault ID: {vaultStateModule.state.identity.id}. Enter your master password to access your passwords."
            />
            <Card>
                <div class="flex flex-col gap-4">
                    {#if error}
                        <Alert tone="danger">{error}</Alert>
                    {/if}
                    <Field id="password" label="Master password" required>
                        {#snippet children(control)}
                            <div class="flex gap-2">
                                <input
                                    {...control}
                                    bind:value={password}
                                    type={showPassword ? "text" : "password"}
                                    disabled={busy}
                                    onkeydown={(e) => e.key === "Enter" && void unlockVault()}
                                    class="flex-1"
                                />
                                <button
                                    type="button"
                                    class="btn btn-ghost btn-md self-end"
                                    onclick={() => showPassword = !showPassword}
                                    aria-label={showPassword ? "Hide password" : "Show password"}
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
                            type="button"
                            disabled={busy}
                            onclick={() => void unlockVault()}
                        >
                            {busy ? "Unlocking…" : "Unlock"}
                        </button>
                    </div>
                </div>
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
