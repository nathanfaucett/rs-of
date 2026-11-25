<script lang="ts">
    import {
        ClientProfile,
        ClientType,
        GrantType,
        ResponseType,
        TokenEndpointAuthMethod,
    } from "@ofsdk/idp-client";
    import { onMount } from "svelte";
    import { goto } from "$app/navigation";
    import { resolve } from "$app/paths";
    import { page } from "$app/state";
    import { createdClients } from "$lib/common/state/createdClients.svelte";
    import {
        type Application,
        idpAdministration,
    } from "$lib/common/state/idpAdministration";
    import { idpApi } from "$lib/common/state/idpClient.svelte";
    import { notifications } from "$lib/common/state/notifications.svelte";
    import { Alert, Card, Field, Page, PageHeader } from "$lib/ui";

    import ApplicationPolicy from "./ApplicationPolicy.svelte";

    const applicationId = page.params.applicationId ?? "";
    if (!applicationId) throw new Error("Application ID is required");

    const profileOptions = Object.values(ClientProfile);

    const defaultAllowedGrantTypes = [
        GrantType.AuthorizationCode,
        GrantType.RefreshToken,
    ];
    const defaultResponseTypes = [ResponseType.Code];

    let loading = $state(false);
    let saving = $state(false);
    let deleting = $state(false);
    let error = $state<string | null>(null);
    let application = $state<Application | null>(null);

    let appName = $state("");
    let appDescription = $state("");

    let creatingClient = $state(false);

    let clientName = $state("");
    let clientUri = $state("");
    let redirectUrisText = $state("");
    let allowedScopesText = $state("");
    let profile = $state(ClientProfile.WebApplication);

    const applicationClients = $derived(
        createdClients.forApplication(applicationId),
    );

    function parseLines(value: string): string[] {
        return value
            .split("\n")
            .map((entry) => entry.trim())
            .filter(Boolean);
    }

    function parseTokens(value: string): string[] {
        return value
            .split(/[\s,]+/)
            .map((entry) => entry.trim())
            .filter(Boolean);
    }

    function resetClientForm() {
        clientName = "";
        clientUri = "";
        redirectUrisText = "";
        allowedScopesText = "";
        profile = ClientProfile.WebApplication;
    }

    async function loadApplication() {
        loading = true;
        error = null;

        try {
            application = await idpAdministration.getApplication({
                applicationId,
            });
            appName = application.name;
            appDescription = application.description ?? "";
        } catch (cause) {
            console.error(cause);
            error =
                cause instanceof Error
                    ? cause.message
                    : "Failed to load application";
            notifications.add("Failed to load application", "error");
        } finally {
            loading = false;
        }
    }

    async function onSaveApplication() {
        if (!application) return;

        saving = true;
        try {
            application = await idpAdministration.updateApplication({
                applicationId,
                applicationRegistration: {
                    name: appName.trim() || undefined,
                    uri: application.uri,
                    description: appDescription.trim() || undefined,
                },
            });
            notifications.add("Application saved", "success");
            appName = application.name;
            appUri = application.uri;
            appDescription = application.description ?? "";
        } catch (cause) {
            console.error(cause);
            notifications.add("Failed to save application", "error");
        } finally {
            saving = false;
        }
    }

    async function onDeleteApplication() {
        if (!confirm("Delete this application?")) {
            return;
        }

        deleting = true;
        try {
            await idpApi.deleteApplication({ applicationId });
            notifications.add("Application deleted", "success");
            await goto(resolve("/applications"));
        } catch (cause) {
            console.error(cause);
            notifications.add("Failed to delete application", "error");
        } finally {
            deleting = false;
        }
    }

    async function onCreateClient(event: SubmitEvent) {
        event.preventDefault();

        if (!application || creatingClient) {
            return;
        }

        if (!clientName.trim()) {
            notifications.add("Client name is required", "error");
            return;
        }

        creatingClient = true;
        const generation = createdClients.generation;
        try {
            const created = await idpApi.register({
                clientRegistration: {
                    application: { uri: application.uri },
                    clientName: clientName.trim(),
                    profile,
                    clientType: ClientType.Public,
                    tokenEndpointAuthMethod: TokenEndpointAuthMethod.None,
                    allowedGrantTypes: defaultAllowedGrantTypes,
                    responseTypes: defaultResponseTypes,
                    redirectUris: parseLines(redirectUrisText),
                    allowedScopes: parseTokens(allowedScopesText),
                    clientUri: clientUri.trim() || undefined,
                },
            });
            if (generation !== createdClients.generation) return;
            createdClients.add(applicationId, created);
            resetClientForm();
            notifications.add("Client created", "success");
        } catch (cause) {
            if (generation !== createdClients.generation) return;
            console.error(cause);
            notifications.add("Failed to create client", "error");
        } finally {
            creatingClient = false;
        }
    }


    onMount(() => {
        void loadApplication();
    });
</script>

<Page>
    <PageHeader title="Application Detail" />

    {#if error}
        <Alert tone="danger">{error}</Alert>
    {:else if loading}
        <Card tone="subtle">Loading application...</Card>
    {:else if application}
        <Card title="Application" description="ID: {application.id}">
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
                            bind:value={appName}
                            type="text"
                        />
                    {/snippet}
                </Field>
                <Field label="URI">
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
                            value={application?.uri}
                            readonly
                            type="text"
                        />
                    {/snippet}
                </Field>
                <Field label="Description">
                    {#snippet children({
                        id,
                        "aria-describedby": describedby,
                        "aria-invalid": invalid,
                        required,
                    })}
                        <textarea
                            {id}
                            aria-describedby={describedby}
                            aria-invalid={invalid}
                            {required}
                            bind:value={appDescription}
                            rows="3"></textarea>
                    {/snippet}
                </Field>
                <div class="flex justify-end gap-2">
                    <button
                        class="btn btn-primary btn-md"
                        type="button"
                        onclick={onSaveApplication}
                        disabled={saving || deleting}
                        >{saving ? "Saving..." : "Save"}</button
                    >
                    <button
                        class="btn btn-danger btn-md"
                        type="button"
                        onclick={onDeleteApplication}
                        disabled={saving || deleting}
                        >{deleting ? "Deleting..." : "Delete"}</button
                    >
                </div>
            </div>
        </Card>

        {#key application.id}
            <ApplicationPolicy applicationId={application.id} />
        {/key}

        <Card
            title="Clients"
            description="Register a public OAuth Client for this application."
        >
            <form class="flex flex-col gap-4" onsubmit={onCreateClient}>
                <Field label="Client name" required>
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
                <Field label="Redirect URIs">
                    {#snippet children({
                        id,
                        "aria-describedby": describedby,
                        "aria-invalid": invalid,
                        required,
                    })}
                        <textarea
                            {id}
                            aria-describedby={describedby}
                            aria-invalid={invalid}
                            {required}
                            bind:value={redirectUrisText}
                            rows="3"></textarea>
                    {/snippet}
                </Field>
                <Field label="Allowed scopes">
                    {#snippet children({
                        id,
                        "aria-describedby": describedby,
                        "aria-invalid": invalid,
                        required,
                    })}
                        <textarea
                            {id}
                            aria-describedby={describedby}
                            aria-invalid={invalid}
                            {required}
                            bind:value={allowedScopesText}
                            rows="2"></textarea>
                    {/snippet}
                </Field>
                <div class="grid gap-3 md:grid-cols-3">
                    <Field label="Profile">
                        {#snippet children({
                            id,
                            "aria-describedby": describedby,
                            "aria-invalid": invalid,
                            required,
                        })}
                            <select
                                {id}
                                aria-describedby={describedby}
                                aria-invalid={invalid}
                                {required}
                                bind:value={profile}
                            >
                                {#each profileOptions as option (option)}
                                    <option value={option}>{option}</option>
                                {/each}
                            </select>
                        {/snippet}
                    </Field>
                </div>
                <div class="flex justify-end">
                    <button
                        class="btn btn-primary btn-md"
                        type="submit"
                        disabled={creatingClient}
                        >{creatingClient
                            ? "Creating..."
                            : "Create client"}</button
                    >
                </div>
            </form>

            <p>
                This list shows only clients created here during this session.
                Reloading clears this list, not the registrations. Save the
                client IDs to <a href={resolve("/clients")}>
                    look up a client by ID
                </a> later. To open a registration, sign in through this application
                with client read permission.
            </p>
            {#if applicationClients.length > 0}
                <div
                    class="overflow-x-auto rounded-card border border-line bg-surface"
                >
                    <table class="table">
                        <thead>
                            <tr>
                                <th>Client ID</th>
                                <th>Name</th>
                                <th>Profile</th>
                                <th>Auth Method</th>
                            </tr>
                        </thead>
                        <tbody>
                            {#each applicationClients as client (client.clientId)}
                                <tr>
                                    <td>
                                        <a
                                            href={resolve(
                                                `/clients/${encodeURIComponent(client.clientId)}?application=${applicationId}`,
                                            )}
                                        >
                                            {client.clientId}
                                        </a>
                                    </td>
                                    <td>{client.clientName}</td>
                                    <td>{client.profile}</td>
                                    <td>{client.tokenEndpointAuthMethod}</td>
                                </tr>
                            {/each}
                        </tbody>
                    </table>
                </div>
            {/if}
        </Card>
    {/if}
</Page>
