<script lang="ts">
import { onMount } from "svelte";
import LocalSecretEditor from "$lib/secrets/LocalSecretEditor.svelte";
import { Alert, Card, Field, Page, PageHeader } from "$lib/ui";
import {
    createVault as createVaultHandle,
    unlockVault as unlockVaultHandle,
    listVaults,
    vaultExists,
    type VaultHandle,
} from "$lib/vault/vault";

type VaultState =
    | { status: "loading" }
    | { status: "select-vault"; vaults: string[] }
    | { status: "create-vault" }
    | { status: "unlock-vault"; name: string }
    | { status: "unlocked"; vault: VaultHandle }
    | { status: "error"; message: string };

let vaultState = $state<VaultState>({ status: "loading" });
let password = $state("");
let confirmPassword = $state("");
let vaultName = $state("");
let error = $state("");
let busy = $state(false);

onMount(() => {
    void showVaultSelector();
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
    const exists = await vaultExists(vaultName.trim());
    if (exists) {
        error = "A vault with this name already exists.";
        return;
    }
    busy = true;
    try {
        const vault = await createVaultHandle(password, vaultName.trim());
        password = "";
        confirmPassword = "";
        vaultName = "";
        vaultState = { status: "unlocked", vault };
    } catch (cause) {
        error = cause instanceof Error ? cause.message : "Failed to create vault.";
    } finally {
        busy = false;
    }
}

async function unlockVault() {
    error = "";
    if (vaultState.status !== "unlock-vault") return;
    if (!password) {
        error = "Password is required.";
        return;
    }
    busy = true;
    try {
        const vault = await unlockVaultHandle(password, vaultState.name);
        password = "";
        vaultState = { status: "unlocked", vault };
    } catch (cause) {
        error = cause instanceof Error ? cause.message : "Failed to unlock vault. Wrong password?";
    } finally {
        busy = false;
    }
}

function selectVault(name: string) {
    error = "";
    password = "";
    vaultState = { status: "unlock-vault", name };
}

function showCreateVault() {
    error = "";
    password = "";
    confirmPassword = "";
    vaultName = "";
    vaultState = { status: "create-vault" };
}

async function showVaultSelector() {
    vaultState = { status: "loading" };
    error = "";
    password = "";
    confirmPassword = "";
    vaultName = "";
    try {
        const vaults = await listVaults();
        vaultState = { status: "select-vault", vaults };
    } catch {
        vaultState = { status: "error", message: "Failed to load vaults." };
    }
}

async function lockVault() {
    if (vaultState.status === "unlocked") {
        vaultState.vault.close();
    }
    await showVaultSelector();
}
</script>

<div class="flex min-h-0 grow items-start justify-center overflow-auto p-6 sm:items-center">
    {#if vaultState.status === "loading"}
        <Page width="narrow">
            <p role="status">Loading…</p>
        </Page>
    {:else if vaultState.status === "select-vault"}
        <Page width="narrow">
            <PageHeader
                title="Password Manager"
                description="Select a vault to unlock, or create a new one."
            />
            <Card>
                <div class="flex flex-col gap-4">
                    {#if vaultState.vaults.length > 0}
                        <div class="flex flex-col gap-2">
                            <p class="text-sm font-medium text-muted-foreground">Existing vaults</p>
                            {#each vaultState.vaults as name}
                                <button
                                    class="btn btn-secondary btn-md text-left"
                                    type="button"
                                    onclick={() => selectVault(name)}
                                >
                                    {name}
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
    {:else if vaultState.status === "create-vault"}
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
                            onclick={showVaultSelector}
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
    {:else if vaultState.status === "unlock-vault"}
        <Page width="narrow">
            <PageHeader
                title="Unlock vault: {vaultState.name}"
                description="Enter your master password to access your passwords."
            />
            <Card>
                <div class="flex flex-col gap-4">
                    {#if error}
                        <Alert tone="danger">{error}</Alert>
                    {/if}
                    <Field id="password" label="Master password" required>
                        {#snippet children(control)}
                            <input
                                {...control}
                                bind:value={password}
                                type="password"
                                disabled={busy}
                                onkeydown={(e) => e.key === "Enter" && void unlockVault()}
                            />
                        {/snippet}
                    </Field>
                    <div class="flex gap-2">
                        <button
                            class="btn btn-secondary btn-md"
                            type="button"
                            disabled={busy}
                            onclick={showVaultSelector}
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
    {:else if vaultState.status === "unlocked"}
        <Page>
            <PageHeader
                title="Password Manager"
                description="Your vault is stored locally and encrypted. This is a local-first app — no server required."
            >
                {#snippet actions()}
                    <button
                        class="btn btn-secondary btn-md"
                        type="button"
                        onclick={lockVault}>Lock vault</button
                    >
                {/snippet}
            </PageHeader>
            <LocalSecretEditor vault={vaultState.vault} />
        </Page>
    {:else if vaultState.status === "error"}
        <Page width="narrow">
            <Alert tone="danger" title="Error">{vaultState.message}</Alert>
            <div class="mt-4">
                <button class="btn btn-secondary btn-md" onclick={showVaultSelector}>Try again</button>
            </div>
        </Page>
    {/if}
</div>
