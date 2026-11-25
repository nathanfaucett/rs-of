<script lang="ts">
    import { onMount } from "svelte";

    import { resolve } from "$app/paths";
    import {
        type Application,
        idpAdministration,
    } from "$lib/common/state/idpAdministration";
    import { notifications } from "$lib/common/state/notifications.svelte";
    import { Alert, Card, Field, Page, PageHeader } from "$lib/ui";

    import { isApplicationUri } from "./applicationUri";

    const limit = 25;

    let loading = $state(false);
    let creating = $state(false);
    let failedOffset = $state<number | null>(null);
    let applications = $state.raw<Application[]>([]);
    let hasNext = $state(false);
    let offset = $state(0);

    let name = $state("");
    let uri = $state("");
    let description = $state("");
    let uriSubmitted = $state(false);
    let uriIssues = $derived(
        uriSubmitted && !isApplicationUri(uri.trim())
            ? [{ message: "Enter an HTTP or HTTPS URL without credentials, or a URN." }]
            : [],
    );

    function formatTimestamp(value: number): string {
        return new Date(value * 1000).toLocaleString();
    }

    async function loadApplications(requestedOffset: number) {
        if (loading) return;
        loading = true;
        failedOffset = null;

        try {
            // The IdP returns rows without a total. Read one extra row to detect an exact end.
            const results = await idpAdministration.listApplications({
                offset: requestedOffset,
                limit: limit + 1,
            });
            hasNext = results.length > limit;
            // Records can disappear between the lookahead and the next request.
            if (requestedOffset > offset && results.length === 0) return;
            applications = results.slice(0, limit);
            offset = requestedOffset;
        } catch (cause) {
            console.error(cause);
            failedOffset = requestedOffset;
        } finally {
            loading = false;
        }
    }

    async function onCreateApplication(event: SubmitEvent) {
        event.preventDefault();

        if (creating || loading) return;

        if (!name.trim()) {
            notifications.add("Application name is required", "error");
            return;
        }

        uriSubmitted = true;
        if (uriIssues.length) return;

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
            uriSubmitted = false;
            description = "";
            notifications.add("Application created", "success");
            await loadApplications(0);
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

        await loadApplications(offset - limit);
    }

    async function onNext() {
        if (loading || creating || failedOffset !== null || !hasNext) {
            return;
        }

        await loadApplications(offset + limit);
    }

    onMount(() => {
        void loadApplications(0);
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
            <Field
                label="URI"
                description="Use an HTTP or HTTPS URL, or a URN. You cannot change it after creation."
                required
                issues={uriIssues}
            >
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

    {#if failedOffset !== null}
        <Alert tone="danger" title="Could not load applications">
            <button
                class="btn btn-secondary btn-sm"
                type="button"
                disabled={creating}
                onclick={() => {
                    if (failedOffset !== null) void loadApplications(failedOffset);
                }}>Retry</button
            >
        </Alert>
    {/if}

    {#if loading && applications.length === 0}
        <Card tone="subtle">Loading applications...</Card>
    {:else if applications.length === 0 && failedOffset === null}
        <Card tone="subtle">No applications found.</Card>
    {:else if applications.length > 0}
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
                            <td data-label="ID">
                                <a
                                    href={resolve(
                                        `/applications/${application.id}`,
                                    )}>{application.id}</a
                                ></td
                            >
                            <td data-label="Name">
                                <a href={resolve(`/applications/${application.id}`)}
                                    >{application.name}</a
                                >
                            </td>
                            <td data-label="URI">{application.uri}</td>
                            <td data-label="Description">{application.description ?? "-"}</td>
                            <td data-label="Updated">{formatTimestamp(application.updated_at)}</td>
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
            disabled={loading || creating || failedOffset !== null || !hasNext}>Next</button
        >
    </div>
</Page>
