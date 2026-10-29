<script lang="ts">
    import { onMount } from "svelte";
    import { resolve } from "$app/paths";
    import {
        type Application,
        idpAdministration,
    } from "$lib/common/state/idpAdministration";

    import { notifications } from "$lib/common/state/notifications.svelte";
    import { Alert, Card, Field, Page, PageHeader } from "$lib/ui";

    const limit = 25;

    let loading = $state(false);
    let creating = $state(false);
    let error = $state<string | null>(null);
    let applications = $state<Application[]>([]);
    let offset = $state(0);

    let name = $state("");
    let uri = $state("");
    let description = $state("");

    function formatTimestamp(value: number): string {
        return new Date(value * 1000).toLocaleString();
    }

    async function loadApplications() {
        loading = true;
        error = null;

        try {
            applications = await idpAdministration.listApplications({
                offset,
                limit,
            });
        } catch (cause) {
            console.error(cause);
            error =
                cause instanceof Error
                    ? cause.message
                    : "Failed to load applications";
            notifications.add("Failed to load applications", "error");
        } finally {
            loading = false;
        }
    }

    async function onCreateApplication(event: SubmitEvent) {
        event.preventDefault();

        if (!name.trim()) {
            notifications.add("Application name is required", "error");
            return;
        }

        if (!uri.trim()) {
            notifications.add("Application URI is required", "error");
            return;
        }

        creating = true;
        try {
            await idpAdministration.createApplication({
                applicationRegistration: {
                    name: name.trim(),
                    uri: uri.trim(),
                    description: description.trim() || undefined,
                },
            });
            name = "";
            uri = "";
            description = "";
            offset = 0;
            notifications.add("Application created", "success");
            await loadApplications();
        } catch (cause) {
            console.error(cause);
            notifications.add("Failed to create application", "error");
        } finally {
            creating = false;
        }
    }

    async function onPrev() {
        if (offset < limit || loading || creating) {
            return;
        }

        offset -= limit;
        await loadApplications();
    }

    async function onNext() {
        if (loading || creating) {
            return;
        }

        offset += limit;
        await loadApplications();
    }

    onMount(() => {
        void loadApplications();
    });
</script>

<Page>
    <PageHeader title="Applications" description="Offset: {offset}" />

    <Card title="Create application">
        <form class="flex flex-col gap-4" onsubmit={onCreateApplication}>
            <Field label="Name" required>
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
                        bind:value={name}
                        type="text"
                    />
                {/snippet}
            </Field>
            <Field label="URI" required>
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
                        bind:value={uri}
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
                        bind:value={description}
                        rows="3"></textarea>
                {/snippet}
            </Field>
            <div class="flex justify-end">
                <button
                    class="btn btn-primary btn-md"
                    type="submit"
                    disabled={creating || loading}
                    >{creating ? "Creating..." : "Create application"}</button
                >
            </div>
        </form>
    </Card>

    {#if error}
        <Alert tone="danger">{error}</Alert>
    {:else if loading}
        <Card tone="subtle">Loading applications...</Card>
    {:else if applications.length === 0}
        <Card tone="subtle">No applications found.</Card>
    {:else}
        <div class="overflow-x-auto rounded-card border border-line bg-surface">
            <table class="table">
                <thead>
                    <tr>
                        <th>ID</th>
                        <th>Name</th>
                        <th>URI</th>
                        <th>Description</th>
                        <th>Updated</th>
                    </tr>
                </thead>
                <tbody>
                    {#each applications as application (application.id)}
                        <tr>
                            <td>
                                <a
                                    href={resolve(
                                        `/applications/${application.id}`,
                                    )}>{application.id}</a
                                ></td
                            >
                            <td>{application.name}</td>
                            <td>
                                <a
                                    href={resolve(
                                        `/applications/${application.id}`,
                                    )}>{application.uri}</a
                                >
                            </td>
                            <td>{application.description ?? "-"}</td>
                            <td>{formatTimestamp(application.updated_at)}</td>
                        </tr>
                    {/each}
                </tbody>
            </table>
        </div>
    {/if}

    <div class="flex items-center justify-end gap-2">
        <button
            class="btn btn-secondary btn-md"
            type="button"
            onclick={onPrev}
            disabled={loading || creating || offset === 0}>Prev</button
        >
        <button
            class="btn btn-secondary btn-md"
            type="button"
            onclick={onNext}
            disabled={loading || creating}>Next</button
        >
    </div>
</Page>
