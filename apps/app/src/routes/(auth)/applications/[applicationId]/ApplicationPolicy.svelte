<script lang="ts">
    import type { PermissionResponse, RoleResponse } from "@ofsdk/management-client";
    import { onMount } from "svelte";
    import { managementApi } from "$lib/common/state/managementClient.svelte";
    import { managementPolicyError, managementPolicyStatus } from "$lib/common/state/managementPolicyError";
    import { notifications } from "$lib/common/state/notifications.svelte";
    import { Alert, Card, Field } from "$lib/ui";

    let { applicationId }: { applicationId: string } = $props();

    let rolesLoading = $state(true);
    let rolesError = $state<{ text: string; retry: boolean } | null>(null);
    let roleWriteDenied = $state(false);
    let roleWriteError = $state<string | null>(null);
    let creatingRole = $state(false);
    let deletingRoleId = $state<string | null>(null);
    let roles = $state.raw<RoleResponse[]>([]);
    let roleName = $state("");
    let roleDescription = $state("");

    let permissionsLoading = $state(true);
    let permissionsError = $state<{ text: string; retry: boolean } | null>(null);
    let permissionWriteDenied = $state(false);
    let permissionWriteError = $state<string | null>(null);
    let creatingPermission = $state(false);
    let deletingPermissionId = $state<string | null>(null);
    let permissions = $state.raw<PermissionResponse[]>([]);
    let permissionName = $state("");
    let permissionDescription = $state("");

    let selectedRoleId = $state("");
    let selectedPermissionId = $state("");
    let rolePermissionsLoading = $state(false);
    let rolePermissionsError = $state<{ text: string; retry: boolean } | null>(null);
    let rolePermissions = $state.raw<PermissionResponse[]>([]);
    let rolePermissionWriteError = $state<string | null>(null);
    let assigningPermission = $state(false);
    let revokingRolePermissionId = $state<string | null>(null);

    function formatTimestamp(value: number): string {
        return new Date(value * 1000).toLocaleString();
    }

    function syncSelections() {
        if (
            selectedRoleId &&
            !roles.some((role) => role.id === selectedRoleId)
        ) {
            selectedRoleId = "";
        }

        if (
            selectedPermissionId &&
            !permissions.some(
                (permission) => permission.id === selectedPermissionId,
            )
        ) {
            selectedPermissionId = "";
        }

        const firstRole = roles[0];
        if (!selectedRoleId && firstRole) {
            selectedRoleId = firstRole.id;
        }

        const firstPermission = permissions[0];
        if (!selectedPermissionId && firstPermission) {
            selectedPermissionId = firstPermission.id;
        }
    }

    async function loadRoles() {
        rolesLoading = true;
        rolesError = null;

        try {
            roles = await managementApi.listRoles({
                applicationId,
                offset: 0,
                limit: 100,
            });
            syncSelections();
        } catch (cause) {
            rolesError = {
                text: managementPolicyError(cause, "list roles"),
                retry: managementPolicyStatus(cause) !== 403,
            };
        } finally {
            rolesLoading = false;
        }
        await loadRolePermissions();
    }

    async function loadPermissions(refreshAssignments = true) {
        permissionsLoading = true;
        permissionsError = null;

        try {
            permissions = await managementApi.listPermissions({
                applicationId,
                offset: 0,
                limit: 100,
            });
            syncSelections();
        } catch (cause) {
            permissionsError = {
                text: managementPolicyError(cause, "list permissions"),
                retry: managementPolicyStatus(cause) !== 403,
            };
        } finally {
            permissionsLoading = false;
        }
        if (refreshAssignments) await loadRolePermissions();
    }

    async function loadRolePermissions() {
        if (rolesLoading || rolesError || permissionsLoading || permissionsError) return;
        const roleId = selectedRoleId;
        rolePermissionsError = null;
        if (!roleId) {
            rolePermissionsLoading = false;
            rolePermissions = [];
            return;
        }

        rolePermissionsLoading = true;
        try {
            const result = await managementApi.listRolePermissions({ applicationId, roleId });
            if (roleId === selectedRoleId) rolePermissions = result;
        } catch (cause) {
            if (roleId !== selectedRoleId) return;
            rolePermissionsError = {
                text: managementPolicyError(cause, "list permissions for this role"),
                retry: managementPolicyStatus(cause) !== 403,
            };
        } finally {
            if (roleId === selectedRoleId) rolePermissionsLoading = false;
        }
    }

    async function onCreateRole(event: SubmitEvent) {
        event.preventDefault();

        if (creatingRole || roleWriteDenied) return;
        if (!roleName.trim()) {
            notifications.add("Role name is required", "error");
            return;
        }

        roleWriteError = null;
        creatingRole = true;
        try {
            await managementApi.createRole({
                applicationId,
                createRoleRequest: {
                    name: roleName.trim(),
                    description: roleDescription.trim() || undefined,
                },
            });
            roleName = "";
            roleDescription = "";
            notifications.add("Role created", "success");
            await loadRoles();
        } catch (cause) {
            roleWriteError = managementPolicyError(cause, "create a role");
            roleWriteDenied = managementPolicyStatus(cause) === 403;
        } finally {
            creatingRole = false;
        }
    }

    async function onDeleteRole(role: RoleResponse) {
        if (!confirm(`Delete role "${role.name}"?`)) {
            return;
        }

        roleWriteError = null;
        deletingRoleId = role.id;
        try {
            await managementApi.deleteRole({
                applicationId,
                roleId: role.id,
            });
            notifications.add("Role deleted", "success");
            await loadRoles();
        } catch (cause) {
            roleWriteError = managementPolicyError(cause, "delete a role");
            roleWriteDenied = managementPolicyStatus(cause) === 403;
        } finally {
            deletingRoleId = null;
        }
    }

    async function onCreatePermission(event: SubmitEvent) {
        event.preventDefault();

        if (creatingPermission || permissionWriteDenied) return;
        if (!permissionName.trim()) {
            notifications.add("Permission name is required", "error");
            return;
        }

        permissionWriteError = null;
        creatingPermission = true;
        try {
            await managementApi.createPermission({
                applicationId,
                createPermissionRequest: {
                    name: permissionName.trim(),
                    description: permissionDescription.trim() || undefined,
                },
            });
            permissionName = "";
            permissionDescription = "";
            notifications.add("Permission created", "success");
            await loadPermissions(false);
        } catch (cause) {
            permissionWriteError = managementPolicyError(cause, "create a permission");
            permissionWriteDenied = managementPolicyStatus(cause) === 403;
        } finally {
            creatingPermission = false;
        }
    }

    async function onDeletePermission(permission: PermissionResponse) {
        if (!confirm(`Delete permission "${permission.name}"?`)) {
            return;
        }

        permissionWriteError = null;
        deletingPermissionId = permission.id;
        try {
            await managementApi.deletePermission({
                applicationId,
                permissionId: permission.id,
            });
            notifications.add("Permission deleted", "success");
            await loadPermissions();
        } catch (cause) {
            permissionWriteError = managementPolicyError(cause, "delete a permission");
            permissionWriteDenied = managementPolicyStatus(cause) === 403;
        } finally {
            deletingPermissionId = null;
        }
    }

    async function onAssignPermissionToRole() {
        const roleId = selectedRoleId;
        const permissionId = selectedPermissionId;

        if (!roleId || !permissionId) {
            notifications.add("Select a role and permission", "error");
            return;
        }

        rolePermissionWriteError = null;
        assigningPermission = true;
        try {
            await managementApi.assignPermissionToRole({
                applicationId,
                roleId,
                permissionId,
            });
            notifications.add("Permission assigned", "success");
            await loadRolePermissions();
        } catch (cause) {
            rolePermissionWriteError = managementPolicyError(cause, "assign a permission");
            permissionWriteDenied = managementPolicyStatus(cause) === 403;
        } finally {
            assigningPermission = false;
        }
    }

    async function onRevokePermissionFromRole(permissionId: string) {
        const roleId = selectedRoleId;
        if (!roleId) {
            return;
        }

        rolePermissionWriteError = null;
        revokingRolePermissionId = permissionId;
        try {
            await managementApi.revokePermissionFromRole({
                applicationId,
                roleId,
                permissionId,
            });
            notifications.add("Permission revoked", "success");
            await loadRolePermissions();
        } catch (cause) {
            rolePermissionWriteError = managementPolicyError(cause, "revoke a permission");
            permissionWriteDenied = managementPolicyStatus(cause) === 403;
        } finally {
            revokingRolePermissionId = null;
        }
    }

    onMount(() => {
        // Do not issue concurrent IdP token exchanges for the initial policy reads.
        void loadRoles().then(() => loadPermissions());
    });
</script>

<Card title="Roles">
    {#if !rolesLoading && !rolesError}
        <form class="flex flex-col gap-4" onsubmit={onCreateRole}>
            <Field label="Name" required>
                {#snippet children(control)}
                    <input
                        {...control}
                        bind:value={roleName}
                        type="text"
                    />
                {/snippet}
            </Field>
            <Field label="Description">
                {#snippet children(control)}
                    <textarea
                        {...control}
                        bind:value={roleDescription}
                        rows="2"></textarea>
                {/snippet}
            </Field>
            <div class="flex justify-end">
                <button
                    class="btn btn-primary btn-md"
                    type="submit"
                    disabled={creatingRole || roleWriteDenied}
                    >{creatingRole ? "Creating..." : "Create role"}</button
                >
            </div>
        </form>
        {#if roleWriteError}<Alert tone="danger">{roleWriteError}</Alert>{/if}
    {/if}

    {#if rolesError}
        <Alert tone="danger">{rolesError.text}</Alert>
        {#if rolesError.retry}
            <button class="btn btn-secondary btn-md" type="button" onclick={loadRoles}>Retry</button>
        {/if}
    {:else if rolesLoading}
        <Card tone="subtle">Loading roles...</Card>
    {:else if roles.length === 0}
        <Card tone="subtle">No roles found.</Card>
    {:else}
        <div
            class="overflow-x-auto rounded-card border border-line bg-surface"
        >
            <table class="table">
                <thead>
                    <tr>
                        <th>ID</th>
                        <th>Name</th>
                        <th>Description</th>
                        <th>Updated</th>
                        <th>Actions</th>
                    </tr>
                </thead>
                <tbody>
                    {#each roles as role (role.id)}
                        <tr>
                            <td data-label="ID">{role.id}</td>
                            <td data-label="Name">{role.name}</td>
                            <td data-label="Description">{role.description ?? "-"}</td>
                            <td data-label="Updated">{formatTimestamp(role.updatedAt)}</td>
                            <td data-label="Actions">
                                <button
                                    class="btn btn-danger btn-md"
                                    type="button"
                                    onclick={() => onDeleteRole(role)}
                                    disabled={roleWriteDenied || deletingRoleId !== null}
                                    >{deletingRoleId === role.id
                                        ? "Deleting..."
                                        : "Delete"}</button
                                >
                            </td>
                        </tr>
                    {/each}
                </tbody>
            </table>
        </div>
    {/if}
</Card>

<Card title="Permissions">
    {#if !permissionsLoading && !permissionsError}
        <form class="flex flex-col gap-4" onsubmit={onCreatePermission}>
            <Field label="Name" required>
                {#snippet children(control)}
                    <input
                        {...control}
                        bind:value={permissionName}
                        type="text"
                    />
                {/snippet}
            </Field>
            <Field label="Description">
                {#snippet children(control)}
                    <textarea
                        {...control}
                        bind:value={permissionDescription}
                        rows="2"></textarea>
                {/snippet}
            </Field>
            <div class="flex justify-end">
                <button
                    class="btn btn-primary btn-md"
                    type="submit"
                    disabled={creatingPermission || permissionWriteDenied}
                    >{creatingPermission
                        ? "Creating..."
                        : "Create permission"}</button
                >
            </div>
        </form>
        {#if permissionWriteError}<Alert tone="danger">{permissionWriteError}</Alert>{/if}
    {/if}

    {#if permissionsError}
        <Alert tone="danger">{permissionsError.text}</Alert>
        {#if permissionsError.retry}
            <button class="btn btn-secondary btn-md" type="button" onclick={() => loadPermissions()}>Retry</button>
        {/if}
    {:else if permissionsLoading}
        <Card tone="subtle">Loading permissions...</Card>
    {:else if permissions.length === 0}
        <Card tone="subtle">No permissions found.</Card>
    {:else}
        <div
            class="overflow-x-auto rounded-card border border-line bg-surface"
        >
            <table class="table">
                <thead>
                    <tr>
                        <th>ID</th>
                        <th>Name</th>
                        <th>Description</th>
                        <th>Updated</th>
                        <th>Actions</th>
                    </tr>
                </thead>
                <tbody>
                    {#each permissions as permission (permission.id)}
                        <tr>
                            <td data-label="ID">{permission.id}</td>
                            <td data-label="Name">{permission.name}</td>
                            <td data-label="Description">{permission.description ?? "-"}</td>
                            <td data-label="Updated">
                                {formatTimestamp(permission.updatedAt)}
                            </td>
                            <td data-label="Actions">
                                <button
                                    class="btn btn-danger btn-md"
                                    type="button"
                                    onclick={() =>
                                        onDeletePermission(permission)}
                                    disabled={permissionWriteDenied || deletingPermissionId !==
                                        null}
                                    >{deletingPermissionId ===
                                    permission.id
                                        ? "Deleting..."
                                        : "Delete"}</button
                                >
                            </td>
                        </tr>
                    {/each}
                </tbody>
            </table>
        </div>
    {/if}
</Card>

<Card title="Role permissions">
    {#if rolesError || permissionsError}
        <p>Load roles and permissions before making assignments.</p>
    {:else}
        <div class="grid gap-3 md:grid-cols-3 md:items-end">
            <Field label="Role">
                {#snippet children(control)}
                    <select
                        {...control}
                        disabled={rolesLoading || rolePermissionsLoading || assigningPermission || revokingRolePermissionId !== null}
                        value={selectedRoleId}
                        onchange={(event) => {
                            selectedRoleId = event.currentTarget.value;
                            void loadRolePermissions();
                        }}
                    >
                        <option value="">Select role</option>
                        {#each roles as role (role.id)}
                            <option value={role.id}
                                >{role.name}</option
                            >
                        {/each}
                    </select>
                {/snippet}
            </Field>
            <Field label="Permission">
                {#snippet children(control)}
                    <select
                        {...control}
                        bind:value={selectedPermissionId}
                    >
                        <option value="">Select permission</option>
                        {#each permissions as permission (permission.id)}
                            <option value={permission.id}
                                >{permission.name}</option
                            >
                        {/each}
                    </select>
                {/snippet}
            </Field>
            <button
                class="btn btn-primary btn-md"
                type="button"
                onclick={onAssignPermissionToRole}
                disabled={permissionWriteDenied || rolePermissionsError !== null || rolePermissionsLoading || !selectedRoleId ||
                    !selectedPermissionId || revokingRolePermissionId !== null ||
                    assigningPermission}
                >{assigningPermission
                    ? "Assigning..."
                    : "Assign permission"}</button
            >
        </div>

        {#if rolePermissionsError}
            <Alert tone="danger">{rolePermissionsError.text}</Alert>
            {#if rolePermissionsError.retry}
                <button class="btn btn-secondary btn-md" type="button" onclick={loadRolePermissions}>Retry</button>
            {/if}
        {:else if rolePermissionsLoading}
            <Card tone="subtle">Loading role permissions...</Card>
        {:else if !selectedRoleId}
            <Card tone="subtle">
                Select a role to view assigned permissions.
            </Card>
        {:else if rolePermissions.length === 0}
            <Card tone="subtle">No permissions assigned to this role.</Card>
        {:else}
            <div
                class="overflow-x-auto rounded-card border border-line bg-surface"
            >
                <table class="table">
                    <thead>
                        <tr>
                            <th>Permission ID</th>
                            <th>Name</th>
                            <th>Description</th>
                            <th>Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        {#each rolePermissions as permission (permission.id)}
                            <tr>
                                <td data-label="Permission ID">{permission.id}</td>
                                <td data-label="Name">{permission.name}</td>
                                <td data-label="Description">{permission.description ?? "-"}</td>
                                <td data-label="Actions">
                                    <button
                                        class="btn btn-danger btn-md"
                                        type="button"
                                        onclick={() =>
                                            onRevokePermissionFromRole(
                                                permission.id,
                                            )}
                                        disabled={permissionWriteDenied || assigningPermission || revokingRolePermissionId !==
                                            null}
                                        >{revokingRolePermissionId ===
                                        permission.id
                                            ? "Revoking..."
                                            : "Revoke"}</button
                                    >
                                </td>
                            </tr>
                        {/each}
                    </tbody>
                </table>
            </div>
        {/if}
        {#if rolePermissionWriteError}<Alert tone="danger">{rolePermissionWriteError}</Alert>{/if}
    {/if}
</Card>
