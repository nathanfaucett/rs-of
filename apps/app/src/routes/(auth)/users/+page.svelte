<script lang="ts">
    import type { UserInfo } from "@aicacia/idp-client";
    import { onMount } from "svelte";
    import { resolve } from "$app/paths";
    import { idpApi } from "$lib/common/state/idpClient.svelte";
    import { notifications } from "$lib/common/state/notifications.svelte";
    import { Alert, Card, Page, PageHeader } from "$lib/ui";

    const pageSize = 25;

    let users = $state<UserInfo[]>([]);
    let loading = $state(false);
    let error = $state<string | null>(null);
    let offset = $state(0);
    let hasNextPage = $state(false);

    const pageNumber = $derived(Math.floor(offset / pageSize) + 1);

    async function loadUsers() {
        loading = true;
        error = null;
        try {
            const result = await idpApi.listUsers({
                offset,
                limit: pageSize + 1,
            });
            hasNextPage = result.length > pageSize;
            users = result.slice(0, pageSize);
        } catch (cause) {
            console.error(cause);
            error =
                cause instanceof Error ? cause.message : "Failed to load users";
            users = [];
            hasNextPage = false;
            notifications.add("Failed to load users", "error");
        } finally {
            loading = false;
        }
    }

    async function onPrev() {
        if (offset === 0 || loading) return;
        offset -= pageSize;
        await loadUsers();
    }

    async function onNext() {
        if (!hasNextPage || loading) return;
        offset += pageSize;
        await loadUsers();
    }

    onMount(() => {
        void loadUsers();
    });
</script>

<Page>
    <PageHeader title="Users" description="Page {pageNumber}" />

    {#if error}
        <Alert tone="danger">{error}</Alert>
    {:else if loading && users.length === 0}
        <Card tone="subtle">Loading users...</Card>
    {:else if users.length === 0}
        <Card tone="subtle">No users found.</Card>
    {:else}
        <div class="overflow-x-auto rounded-card border border-line bg-surface">
            <table class="table">
                <thead>
                    <tr>
                        <th>User ID</th>
                        <th>Name</th>
                        <th>Username</th>
                        <th>Email</th>
                        <th>Updated</th>
                    </tr>
                </thead>
                <tbody>
                    {#each users as user (user.sub)}
                        <tr>
                            <td>
                                <a href={resolve(`/users/${user.sub}`)}
                                    >{user.sub}</a
                                >
                            </td>
                            <td>{user.name ?? "-"}</td>
                            <td>{user.preferredUsername ?? "-"}</td>
                            <td>{user.email ?? "-"}</td>
                            <td>{user.updatedAt.toLocaleString()}</td>
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
            disabled={loading || offset === 0}>Prev</button
        >
        <button
            class="btn btn-secondary btn-md"
            type="button"
            onclick={onNext}
            disabled={loading || !hasNextPage}>Next</button
        >
    </div>
</Page>
