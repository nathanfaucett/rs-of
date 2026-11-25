<script lang="ts">
    import { ResponseError, type UserInfo } from "@ofsdk/idp-client";
    import { onMount } from "svelte";
    import { resolve } from "$app/paths";
    import Modal from "$lib/common/components/Modal.svelte";
    import { idpApi } from "$lib/common/state/idpClient.svelte";
    import { notifications } from "$lib/common/state/notifications.svelte";
    import { Alert, Card, Field, Page, PageHeader } from "$lib/ui";

    const pageSize = 25;

    let users = $state<UserInfo[]>([]);
    let loading = $state(false);
    let error = $state<string | null>(null);
    let offset = $state(0);
    let hasNextPage = $state(false);

    let createModalRef: Modal;
    let creating = $state(false);
    let createError = $state<string | null>(null);
    let newName = $state("");
    let newEmail = $state("");
    let newPassword = $state("");

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

    function openCreateModal() {
        newName = "";
        newEmail = "";
        newPassword = "";
        createError = null;
        createModalRef?.show();
    }

    async function onCreateUser(event: SubmitEvent) {
        event.preventDefault();
        if (creating) return;
        creating = true;
        createError = null;
        try {
            await idpApi.createUser({
                createUserRequest: {
                    name: newName,
                    email: newEmail,
                    password: newPassword,
                },
            });
            newPassword = "";
            notifications.add("User created", "success");
            createModalRef?.close();
            offset = 0;
            await loadUsers();
        } catch (cause) {
            const status =
                cause instanceof ResponseError ? cause.response.status : null;
            if (status === 403) {
                createError =
                    "You do not have permission to create users. Ask an administrator for access.";
            } else if (status === 401) {
                createError = "Sign in again to create users.";
            } else if (status === 400 || status === 422) {
                createError =
                    "The service rejected the user details. Check the fields and try again.";
            } else {
                createError =
                    "Could not confirm user creation. Check the Users list before you try again.";
            }
        } finally {
            creating = false;
        }
    }

    onMount(() => {
        void loadUsers();
    });
</script>

<Page>
    <PageHeader title="Users" description="Page {pageNumber}">
        {#snippet actions()}
            <button
                class="btn btn-primary btn-md"
                type="button"
                onclick={openCreateModal}
            >
                Add user
            </button>
        {/snippet}
    </PageHeader>

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

    <Modal bind:this={createModalRef} title="Create User">
        <form class="flex flex-col gap-4" onsubmit={onCreateUser}>
            <Field label="Name" required>
                {#snippet children(control)}
                    <input
                        type="text"
                        class="input"
                        bind:value={newName}
                        {...control}
                    />
                {/snippet}
            </Field>
            <Field label="Email" required>
                {#snippet children(control)}
                    <input
                        type="email"
                        class="input"
                        bind:value={newEmail}
                        {...control}
                    />
                {/snippet}
            </Field>
            <Field label="Password" description="Use at least 8 characters." required>
                {#snippet children(control)}
                    <input
                        type="password"
                        class="input"
                        bind:value={newPassword}
                        minlength={8}
                        autocomplete="new-password"
                        {...control}
                    />
                {/snippet}
            </Field>
            {#if createError}
                <Alert tone="danger">{createError}</Alert>
            {/if}
            <div class="flex justify-end gap-2">
                <button
                    class="btn btn-secondary btn-md"
                    type="button"
                    onclick={() => createModalRef?.close()}
                    disabled={creating}>Cancel</button
                >
                <button
                    class="btn btn-primary btn-md"
                    type="submit"
                    disabled={creating}
                >
                    {creating ? "Creating..." : "Create"}
                </button>
            </div>
        </form>
    </Modal>
</Page>
