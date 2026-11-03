<script lang="ts">
import { onMount } from "svelte";
import LocalSecretEditor from "$lib/components/secrets/LocalSecretEditor.svelte";
import { VaultFactory, type VaultHandle } from "$lib/vault/vault";
import { Alert, Card, Field, Page, PageHeader } from "$lib/ui";

type VaultState =
    | { status: "loading" }
    | { status: "no-vault" }
    | { status: "locked" }
    | { status: "unlocked"; vault: VaultHandle }
    | { status: "error"; message: string };

let state = $state<VaultState>({ status: "loading" });
let password = $state("");
let confirmPassword = $state("");
let error = $state("");
let busy = $state(false);

onMount(async () => {
    try {
        const exists = await VaultFactory.exists();
        state = exists ? { status: "locked" } : { status: "no-vault" };
    } catch {
        state = { status: "error", message: "Failed to check vault status." };
    }
});

async function createVault() {
    error = "";
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
    busy = true;
    try {
        const vault = await VaultFactory.create(password);
        password = "";
        confirmPassword = "";
        state = { status: "unlocked", vault };
    } catch (cause) {
        error = cause instanceof Error ? cause.message : "Failed to create vault.";
    } finally {
        busy = false;
    }
}

async function unlockVault() {
    error = "";
    if (!password) {
        error = "Password is required.";
        return;
    }
    busy = true;
    try {
        const vault = await VaultFactory.unlock(password);
        password = "";
        state = { status: "unlocked", vault };
    } catch (cause) {
        error = cause instanceof Error ? cause.message : "Failed to unlock vault. Wrong password?";
    } finally {
        busy = false;
    }
}

function lockVault() {
    if (state.status === "unlocked") {
        state.vault.close();
    }
    state = { status: "locked" };
    password = "";
    confirmPassword = "";
    error = "";
}
</script>

<div class="flex min-h-0 grow items-start justify-center overflow-auto p-6 sm:items-center">
    {#if state.status === "loading"}
        <Page width="narrow">
            <p role="status">Loading…</p>
        </Page>
    {:else if state.status === "no-vault"}
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
                    <button
                        class="btn btn-primary btn-md"
                        type="button"
                        disabled={busy}
                        onclick={() => void createVault()}
                    >
                        {busy ? "Creating…" : "Create vault"}
                    </button>
                </div>
            </Card>
        </Page>
    {:else if state.status === "locked"}
        <Page width="narrow">
            <PageHeader
                title="Unlock your vault"
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
                    <button
                        class="btn btn-primary btn-md"
                        type="button"
                        disabled={busy}
                        onclick={() => void unlockVault()}
                    >
                        {busy ? "Unlocking…" : "Unlock"}
                    </button>
                </div>
            </Card>
        </Page>
    {:else if state.status === "unlocked"}
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
            <LocalSecretEditor vault={state.vault} />
        </Page>
    {:else if state.status === "error"}
        <Page width="narrow">
            <Alert tone="danger" title="Error">{state.message}</Alert>
        </Page>
    {/if}
</div>
