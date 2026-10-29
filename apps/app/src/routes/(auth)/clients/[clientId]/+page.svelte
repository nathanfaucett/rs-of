<script lang="ts">
    import type { ClientRegistration } from "@aicacia/idp-client";
    import { onMount } from "svelte";
    import { goto } from "$app/navigation";
    import { resolve } from "$app/paths";
    import { page } from "$app/state";
    import {
        type ClientKey,
        idpAdministration,
    } from "$lib/common/state/idpAdministration";
    import { idpApi } from "$lib/common/state/idpClient.svelte";
    import { notifications } from "$lib/common/state/notifications.svelte";
    import { Alert, Card, Field, Page, PageHeader } from "$lib/ui";

    const clientId = page.params.clientId;

    let loading = $state(false);
    let saving = $state(false);
    let deleting = $state(false);
    let error = $state<string | null>(null);
    let client = $state<ClientRegistration | null>(null);
    let keysLoading = $state(false);
    let keysError = $state<string | null>(null);
    let keys = $state<ClientKey[]>([]);
    let jwkLoadingKeyId = $state<string | null>(null);
    let jwkError = $state<string | null>(null);
    let jwk = $state<Record<string, unknown> | null>(null);
    let jwkKeyId = $state<string | null>(null);

    let clientName = $state("");
    let clientUri = $state("");
    let logoUri = $state("");
    let policyUri = $state("");
    let tosUri = $state("");

    function formatTimestamp(value?: number | null): string {
        if (value == null) {
            return "-";
        }

        return new Date(value * 1000).toLocaleString();
    }

    function applyClient(currentClient: ClientRegistration) {
        client = currentClient;
        clientName = currentClient.clientName ?? "";
        clientUri = currentClient.clientUri ?? "";
        logoUri = currentClient.logoUri ?? "";
        policyUri = currentClient.policyUri ?? "";
        tosUri = currentClient.tosUri ?? "";
    }

    async function loadClientKeys() {
        keysLoading = true;
        keysError = null;
        try {
            keys = await idpAdministration.listClientKeys({ clientId });
        } catch (cause) {
            console.error(cause);
            keysError =
                cause instanceof Error
                    ? cause.message
                    : "Failed to load client keys";
            notifications.add("Failed to load client keys", "error");
        } finally {
            keysLoading = false;
        }
    }

    async function loadClient() {
        loading = true;
        error = null;

        try {
            const currentClient = await idpApi.getRegister({
                clientId,
            });
            applyClient(currentClient);
            void loadClientKeys();
        } catch (cause) {
            console.error(cause);
            error =
                cause instanceof Error
                    ? cause.message
                    : "Failed to load client";
            notifications.add("Failed to load client", "error");
        } finally {
            loading = false;
        }
    }

    async function onLoadJwk(key: ClientKey) {
        jwkLoadingKeyId = key.id;
        jwkError = null;
        try {
            jwk = key.public_jwk;
            if (!jwk) throw new Error("This key has no public JWK");
            jwkKeyId = key.id;
            notifications.add("JWK loaded", "success");
        } catch (cause) {
            console.error(cause);
            jwk = null;
            jwkKeyId = null;
            jwkError =
                cause instanceof Error ? cause.message : "Failed to load JWK";
            notifications.add("Failed to load JWK", "error");
        } finally {
            jwkLoadingKeyId = null;
        }
    }

    async function onSave() {
        if (!client) {
            return;
        }

        saving = true;
        try {
            await idpApi.putRegister({
                clientId,
                clientRegistration: {
                    ...client,
                    clientName,
                    clientUri: clientUri || undefined,
                    logoUri: logoUri || undefined,
                    policyUri: policyUri || undefined,
                    tosUri: tosUri || undefined,
                },
            });
            notifications.add("Client saved", "success");
            await loadClient();
        } catch (cause) {
            console.error(cause);
            notifications.add("Failed to save client", "error");
        } finally {
            saving = false;
        }
    }

    async function onDeleteClient() {
        if (!confirm("Delete this client?")) {
            return;
        }

        deleting = true;
        try {
            await idpApi.deleteRegister({ clientId });
            notifications.add("Client deleted", "success");
            await goto(resolve("/clients"));
        } catch (cause) {
            console.error(cause);
            notifications.add("Failed to delete client", "error");
        } finally {
            deleting = false;
        }
    }

    onMount(() => {
        void loadClient();
    });
</script>

<Page>
    <PageHeader title="Client Detail" />

    {#if error}
        <Alert tone="danger">{error}</Alert>
    {:else if loading}
        <Card tone="subtle">Loading client...</Card>
    {:else if client}
        <Card description="Client ID: {client.clientId ?? '-'}">
            <div class="flex flex-col gap-4">
                <Field label="Name">
                    {#snippet children({
                        id,
                        "aria-describedby": describedby,
                        "aria-invalid": invalid,
                        required,
                    })}
                        <input
                            {id}
                            aria-describedby={describedby}
                            aria-invalid={invalid}
                            {required}
                            bind:value={clientName}
                            type="text"
                        />
                    {/snippet}
                </Field>
                <Field label="Client URI">
                    {#snippet children({
                        id,
                        "aria-describedby": describedby,
                        "aria-invalid": invalid,
                        required,
                    })}
                        <input
                            {id}
                            aria-describedby={describedby}
                            aria-invalid={invalid}
                            {required}
                            bind:value={clientUri}
                            type="url"
                        />
                    {/snippet}
                </Field>
                <Field label="Logo URI">
                    {#snippet children({
                        id,
                        "aria-describedby": describedby,
                        "aria-invalid": invalid,
                        required,
                    })}
                        <input
                            {id}
                            aria-describedby={describedby}
                            aria-invalid={invalid}
                            {required}
                            bind:value={logoUri}
                            type="url"
                        />
                    {/snippet}
                </Field>
                <Field label="Policy URI">
                    {#snippet children({
                        id,
                        "aria-describedby": describedby,
                        "aria-invalid": invalid,
                        required,
                    })}
                        <input
                            {id}
                            aria-describedby={describedby}
                            aria-invalid={invalid}
                            {required}
                            bind:value={policyUri}
                            type="url"
                        />
                    {/snippet}
                </Field>
                <Field label="Terms of service URI">
                    {#snippet children({
                        id,
                        "aria-describedby": describedby,
                        "aria-invalid": invalid,
                        required,
                    })}
                        <input
                            {id}
                            aria-describedby={describedby}
                            aria-invalid={invalid}
                            {required}
                            bind:value={tosUri}
                            type="url"
                        />
                    {/snippet}
                </Field>
                <div class="flex justify-end">
                    <button
                        class="btn btn-primary btn-md"
                        type="button"
                        onclick={onSave}
                        disabled={saving || deleting}
                        >{saving ? "Saving..." : "Save"}</button
                    >
                </div>
            </div>
        </Card>

        <Card title="Keys">
            {#if keysError}
                <Alert tone="danger">{keysError}</Alert>
            {:else if keysLoading}
                <Card tone="subtle">Loading keys...</Card>
            {:else if keys.length === 0}
                <Card tone="subtle">No keys found.</Card>
            {:else}
                <div
                    class="overflow-x-auto rounded-card border border-line bg-surface"
                >
                    <table class="table">
                        <thead>
                            <tr>
                                <th>ID</th>
                                <th>Name</th>
                                <th>Derivation path</th>
                                <th>Hardened</th>
                                <th>Revoked</th>
                                <th>Expires</th>
                                <th>Actions</th>
                            </tr>
                        </thead>
                        <tbody>
                            {#each keys as key (key.id)}
                                <tr>
                                    <td>{key.id}</td>
                                    <td>{key.name}</td>
                                    <td>{key.derivation_path}</td>
                                    <td>{key.hardened ? "Yes" : "No"}</td>
                                    <td>
                                        {key.revoked_at
                                            ? formatTimestamp(key.revoked_at)
                                            : "Active"}
                                    </td>
                                    <td>
                                        {key.expires_at
                                            ? formatTimestamp(key.expires_at)
                                            : "No expiry"}
                                    </td>
                                    <td>
                                        <button
                                            class="btn btn-secondary btn-md"
                                            type="button"
                                            onclick={() => onLoadJwk(key)}
                                            disabled={jwkLoadingKeyId !== null}
                                            >{jwkLoadingKeyId === key.id
                                                ? "Loading..."
                                                : "View JWK"}</button
                                        >
                                    </td>
                                </tr>
                            {/each}
                        </tbody>
                    </table>
                </div>
            {/if}

            {#if jwkError}
                <Alert tone="danger">{jwkError}</Alert>
            {:else if jwkLoadingKeyId !== null}
                <Card tone="subtle">Loading JWK...</Card>
            {:else if jwk && jwkKeyId !== null}
                <div class="flex flex-col gap-2">
                    <p class="mb-0 text-sm text-ink-muted">
                        JWK for key {jwkKeyId}
                    </p>
                    <pre
                        class="overflow-x-auto rounded-control border border-line bg-surface-muted p-4 text-sm">{JSON.stringify(
                            jwk,
                            null,
                            2,
                        )}</pre>
                </div>
            {/if}
        </Card>

        {#if client.redirectUris && client.redirectUris.length > 0}
            <Card title="Redirect URIs">
                <ul>
                    {#each client.redirectUris as uri (uri)}
                        <li>{uri}</li>
                    {/each}
                </ul>
            </Card>
        {/if}

        {#if client.allowedScopes && client.allowedScopes.length > 0}
            <Card title="Allowed scopes">
                <ul>
                    {#each client.allowedScopes as scope (scope)}
                        <li>{scope}</li>
                    {/each}
                </ul>
            </Card>
        {/if}

        <Card
            title="Danger zone"
            description="Deleting this client cannot be undone."
        >
            <div class="flex justify-end">
                <button
                    class="btn btn-danger btn-md"
                    type="button"
                    onclick={onDeleteClient}
                    disabled={deleting || saving}
                    >{deleting ? "Deleting..." : "Delete client"}</button
                >
            </div>
        </Card>
    {/if}
</Page>
