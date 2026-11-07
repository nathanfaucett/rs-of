<script lang="ts">
import { onMount, tick } from "svelte";
import { beforeNavigate } from "$app/navigation";
import { redirect } from "@sveltejs/kit";
import RestoreBackup from "$lib/vault/RestoreBackup.svelte";
import { createInactivityLock, DEFAULT_LOCK_SETTINGS, type LockSettings } from "$lib/vault/lock";
import { Alert, Card, Field, Page, PageHeader } from "$lib/ui";
import { Eye, EyeOff } from "@lucide/svelte";
import {
    createVault as createVaultHandle,
    listVaults,
    unlockVault as unlockVaultHandle,
    type VaultHandle,
    type VaultIdentity,
} from "$lib/vault/vault";

type VaultState =
    | { status: "loading" }
    | { status: "select-vault"; vaults: VaultIdentity[] }
    | { status: "create-vault" }
    | { status: "restore-vault" }
    | { status: "unlock-vault"; identity: VaultIdentity }
    | { status: "unlocked"; vault: VaultHandle }
    | { status: "error"; message: string };

let vaultState = $state<VaultState>({ status: "loading" });
let password = $state("");
let confirmPassword = $state("");
let showPassword = $state(false);
let vaultName = $state("");
let error = $state("");
let busy = $state(false);
let notice = $state("");
let session = 0;
let lockSettings = $state<LockSettings>({ ...DEFAULT_LOCK_SETTINGS });
const inactivity = createInactivityLock(() => { void lockVault(); });

function settingsSaved(settings: LockSettings) {
    if (inactivity.check()) return;
    lockSettings = settings;
    inactivity.start(settings);
}

async function enterVault(vault: VaultHandle, request: number) {
    try {
        const settings = await vault.loadLockSettings();
        if (request !== session) { vault.close(); return; }
        lockSettings = settings;
        inactivity.start(settings);
        // Redirect to vault page instead of setting unlocked state
        redirect(302, "/vault");
    } catch (cause) {
        vault.close();
        throw cause;
    }
}

beforeNavigate((navigation) => {
    if (!busy && !itemEditor?.hasUnsavedChanges()) return;
    if (navigation.willUnload || !window.confirm("Leave with unsaved or pending changes?")) navigation.cancel();
});

async function restored(identity: VaultIdentity) {
    await showVaultSelector();
    notice = `Restore verified: ${identity.name}. ID: ${identity.id}. Unlock it and check the data before discarding the backup.`;
}

let itemEditor: { hasUnsavedChanges: () => boolean; retire: () => void } | undefined;

onMount(() => {
    void showVaultSelector();
    const activity = (event: Event) => {
        if (inactivity.check()) {
            event.preventDefault();
            event.stopImmediatePropagation();
            return;
        }
        inactivity.activity();
    };
    const events = ["pointerdown", "pointermove", "keydown", "input", "click", "wheel"];
    for (const event of events) window.addEventListener(event, activity, { capture: true });
    const timer = setInterval(() => inactivity.check(), 1000);
    return () => {
        clearInterval(timer);
        for (const event of events) window.removeEventListener(event, activity, { capture: true });
        inactivity.stop();
        session++;
        password = "";
        confirmPassword = "";
        itemEditor?.retire();
        if (vaultState.status === "unlocked") vaultState.vault.close();
    };
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
    const request = session;
    try {
        const vault = await createVaultHandle(password, vaultName.trim());
        if (request !== session) { vault.close(); return; }
        password = "";
        confirmPassword = "";
        vaultName = "";
        await enterVault(vault, request);
    } catch (cause) {
        if (request === session) error = `${cause instanceof Error ? cause.message : "Failed to create vault."} Check browser storage and free space. Return to the selector to recover an interrupted registration before retrying. Do not clear browser data.`;
    } finally {
        if (request === session) busy = false;
    }
}

async function unlockVault() {
    error = "";
    if (busy || vaultState.status !== "unlock-vault") return;
    if (!password) {
        error = "Password is required.";
        return;
    }
    busy = true;
    const request = session;
    try {
        const vault = await unlockVaultHandle(password, vaultState.identity.id);
        if (request !== session) { vault.close(); return; }
        password = "";
        await enterVault(vault, request);
    } catch (cause) {
        if (request === session) error = cause instanceof Error ? cause.message : "Failed to unlock vault. Wrong password?";
    } finally {
        if (request === session) busy = false;
    }
}

function selectVault(identity: VaultIdentity) {
    notice = "";
    error = "";
    password = "";
    vaultState = { status: "unlock-vault", identity };
}

function showCreateVault() {
    error = "";
    password = "";
    confirmPassword = "";
    vaultName = "";
    vaultState = { status: "create-vault" };
}

async function showVaultSelector() {
    notice = "";
    vaultState = { status: "loading" };
    error = "";
    password = "";
    confirmPassword = "";
    vaultName = "";
    const request = session;
    try {
        const vaults = await listVaults();
        if (request === session) vaultState = { status: "select-vault", vaults };
    } catch (cause) {
        if (request === session) vaultState = { status: "error", message: cause instanceof Error ? cause.message : "Cannot load local vaults. Allow browser storage and reload. Do not clear browser data." };
    }
}

async function lockVault() {
    inactivity.stop();
    session++;
    lockSettings = { ...DEFAULT_LOCK_SETTINGS };
    password = "";
    confirmPassword = "";
    notice = "";
    error = "";
    busy = false;
    itemEditor?.retire();
    if (vaultState.status === "unlocked") vaultState.vault.close();
    itemEditor = undefined;
    const request = session;
    await showVaultSelector();
    if (request !== session) return;
    notice = "Vault locked. Unsaved edits were discarded. A write already sent may have persisted; unlock and verify before retrying. Copied values can remain on the clipboard.";
}
</script>

<svelte:window onfocus={() => inactivity.check()} onpageshow={() => inactivity.check()} />
<svelte:document onvisibilitychange={() => inactivity.check()} />

<div class="flex min-h-0 grow items-start justify-center overflow-auto p-6">
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
            {#if notice}<p role="status">{notice}</p>{/if}
            <Card>
                <div class="flex flex-col gap-4">
                    <button class="btn btn-secondary btn-md" type="button" onclick={() => { notice = ""; vaultState = { status: "restore-vault" }; }}>Restore encrypted backup</button>
                    {#if vaultState.vaults.length > 0}
                        <div class="flex flex-col gap-2">
                            <p class="text-sm font-medium text-muted-foreground">Existing vaults</p>
                            {#each vaultState.vaults as identity (identity.id)}
                                <button
                                    class="btn btn-secondary btn-md text-left"
                                    type="button"
                                    onclick={() => selectVault(identity)}
                                >
                                    <span class="vault-name">{identity.name}</span>
                                    <span class="block break-all text-xs">ID: {identity.id}</span>
                                    {#if identity.status === "pending"}
                                        <span class="block text-xs">Registration interrupted. Unlock to recover. If no data was saved, create a new vault.</span>
                                    {/if}
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
    {:else if vaultState.status === "restore-vault"}
        <Page width="narrow">
            <RestoreBackup onrestored={restored} oncancel={() => void showVaultSelector()} />
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
                title="Unlock vault: {vaultState.identity.name}"
                description="Vault ID: {vaultState.identity.id}. Enter your master password to access your passwords."
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
    {:else if vaultState.status === "error"}
        <Page width="narrow">
            <Alert tone="danger" title="Error">{vaultState.message}</Alert>
            <div class="mt-4">
                <button type="button" class="btn btn-secondary btn-md" onclick={showVaultSelector}>Try again</button>
            </div>
        </Page>
    {/if}
</div>
