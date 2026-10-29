<script lang="ts">
    import {
        ClientProfile,
        type ClientRegistration,
        ClientType,
        GrantType,
        ResponseType,
        TokenEndpointAuthMethod,
    } from "@aicacia/idp-client";
    import type {
        PermissionResponse,
        RoleResponse,
    } from "@aicacia/management-client";
    import { onMount } from "svelte";
    import { goto } from "$app/navigation";
    import { resolve } from "$app/paths";
    import { page } from "$app/state";
    import {
        type Application,
        idpAdministration,
    } from "$lib/common/state/idpAdministration";
    import { idpApi } from "$lib/common/state/idpClient.svelte";
    import { managementApi } from "$lib/common/state/managementClient.svelte";
    import { notifications } from "$lib/common/state/notifications.svelte";
    import { Alert, Card, Field, Page, PageHeader } from "$lib/ui";

    const applicationId = page.params.applicationId;
    const profileOptions = Object.values(ClientProfile);
    const clientTypeOptions = Object.values(ClientType);
    const tokenEndpointAuthMethodOptions = Object.values(
        TokenEndpointAuthMethod,
    );
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
    let appUri = $state("");
    let appDescription = $state("");

    let rolesLoading = $state(false);
    let rolesError = $state<string | null>(null);
    let creatingRole = $state(false);
    let deletingRoleId = $state<string | null>(null);
    let roles = $state<RoleResponse[]>([]);
    let roleName = $state("");
    let roleDescription = $state("");

    let permissionsLoading = $state(false);
    let permissionsError = $state<string | null>(null);
    let creatingPermission = $state(false);
    let deletingPermissionId = $state<string | null>(null);
    let permissions = $state<PermissionResponse[]>([]);
    let permissionName = $state("");
    let permissionDescription = $state("");

    let selectedRoleId = $state("");
    let selectedPermissionId = $state("");
    let rolePermissionsLoading = $state(false);
    let rolePermissionsError = $state<string | null>(null);
    let rolePermissions = $state<PermissionResponse[]>([]);
    let assigningPermission = $state(false);
    let revokingRolePermissionId = $state<string | null>(null);

    let creatingClient = $state(false);
    let clients = $state<ClientRegistration[]>([]);

    let clientName = $state("");
    let clientUri = $state("");
    let redirectUrisText = $state("");
    let allowedScopesText = $state("");
    let profile = $state(ClientProfile.WebApplication);
    let clientType = $state(ClientType.Public);
    let tokenEndpointAuthMethod = $state(TokenEndpointAuthMethod.None);

    const applicationClients = $derived(
        application
            ? clients.filter(
                  (client) => client.application.uri === application!.uri,
              )
            : [],
    );

    function formatTimestamp(value: number): string {
        return new Date(value * 1000).toLocaleString();
    }

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

    function syncSelections() {
        if (
            selectedRoleId &&
            !roles.some((role) => String(role.id) === selectedRoleId)
        ) {
            selectedRoleId = "";
        }

        if (
            selectedPermissionId &&
            !permissions.some(
                (permission) => String(permission.id) === selectedPermissionId,
            )
        ) {
            selectedPermissionId = "";
        }

        if (!selectedRoleId && roles.length > 0) {
            selectedRoleId = String(roles[0].id);
        }

        if (!selectedPermissionId && permissions.length > 0) {
            selectedPermissionId = String(permissions[0].id);
        }
    }

    function resetClientForm() {
        clientName = "";
        clientUri = "";
        redirectUrisText = "";
        allowedScopesText = "";
        profile = ClientProfile.WebApplication;
        clientType = ClientType.Public;
        tokenEndpointAuthMethod = TokenEndpointAuthMethod.None;
    }

    async function loadApplication() {
        loading = true;
        error = null;

        try {
            application = await idpAdministration.getApplication({
                applicationId,
            });
            appName = application.name;
            appUri = application.uri;
            appDescription = application.description ?? "";
            await Promise.all([loadRoles(), loadPermissions()]);
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

    async function loadRoles() {
        rolesLoading = true;
        rolesError = null;

        try {
            roles = await managementApi.listRoles({
                applicationId,
                offset: 0,
                limit: 200,
            });
            syncSelections();
            if (selectedRoleId) {
                await loadRolePermissions();
            } else {
                rolePermissions = [];
            }
        } catch (cause) {
            console.error(cause);
            rolesError =
                cause instanceof Error ? cause.message : "Failed to load roles";
            notifications.add("Failed to load roles", "error");
        } finally {
            rolesLoading = false;
        }
    }

    async function loadPermissions() {
        permissionsLoading = true;
        permissionsError = null;

        try {
            permissions = await managementApi.listPermissions({
                applicationId,
                offset: 0,
                limit: 200,
            });
            syncSelections();
        } catch (cause) {
            console.error(cause);
            permissionsError =
                cause instanceof Error
                    ? cause.message
                    : "Failed to load permissions";
            notifications.add("Failed to load permissions", "error");
        } finally {
            permissionsLoading = false;
        }
    }

    async function loadRolePermissions() {
        if (!selectedRoleId) {
            rolePermissions = [];
            return;
        }

        const roleId = selectedRoleId;
        if (!roleId) {
            rolePermissions = [];
            return;
        }

        rolePermissionsLoading = true;
        rolePermissionsError = null;
        try {
            rolePermissions = await managementApi.listRolePermissions({
                applicationId,
                roleId,
            });
        } catch (cause) {
            console.error(cause);
            rolePermissionsError =
                cause instanceof Error
                    ? cause.message
                    : "Failed to load role permissions";
            notifications.add("Failed to load role permissions", "error");
        } finally {
            rolePermissionsLoading = false;
        }
    }

    async function onSaveApplication() {
        saving = true;
        try {
            await idpAdministration.updateApplication({
                applicationId,
                applicationRegistration: {
                    name: appName.trim() || undefined,
                    uri: appUri.trim(),
                    description: appDescription.trim() || undefined,
                },
            });
            notifications.add("Application saved", "success");
            await loadApplication();
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

    async function onCreateRole(event: SubmitEvent) {
        event.preventDefault();

        if (!roleName.trim()) {
            notifications.add("Role name is required", "error");
            return;
        }

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
            console.error(cause);
            notifications.add("Failed to create role", "error");
        } finally {
            creatingRole = false;
        }
    }

    async function onDeleteRole(role: RoleResponse) {
        if (!confirm(`Delete role "${role.name}"?`)) {
            return;
        }

        deletingRoleId = role.id;
        try {
            await managementApi.deleteRole({
                applicationId,
                roleId: role.id,
            });
            notifications.add("Role deleted", "success");
            await loadRoles();
        } catch (cause) {
            console.error(cause);
            notifications.add("Failed to delete role", "error");
        } finally {
            deletingRoleId = null;
        }
    }

    async function onCreatePermission(event: SubmitEvent) {
        event.preventDefault();

        if (!permissionName.trim()) {
            notifications.add("Permission name is required", "error");
            return;
        }

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
            await loadPermissions();
            await loadRolePermissions();
        } catch (cause) {
            console.error(cause);
            notifications.add("Failed to create permission", "error");
        } finally {
            creatingPermission = false;
        }
    }

    async function onDeletePermission(permission: PermissionResponse) {
        if (!confirm(`Delete permission "${permission.name}"?`)) {
            return;
        }

        deletingPermissionId = permission.id;
        try {
            await managementApi.deletePermission({
                applicationId,
                permissionId: permission.id,
            });
            notifications.add("Permission deleted", "success");
            await loadPermissions();
            await loadRolePermissions();
        } catch (cause) {
            console.error(cause);
            notifications.add("Failed to delete permission", "error");
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
            console.error(cause);
            notifications.add("Failed to assign permission", "error");
        } finally {
            assigningPermission = false;
        }
    }

    async function onRevokePermissionFromRole(permissionId: string) {
        const roleId = selectedRoleId;
        if (!roleId) {
            return;
        }

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
            console.error(cause);
            notifications.add("Failed to revoke permission", "error");
        } finally {
            revokingRolePermissionId = null;
        }
    }

    async function onCreateClient(event: SubmitEvent) {
        event.preventDefault();

        if (!application) {
            return;
        }

        if (!clientName.trim()) {
            notifications.add("Client name is required", "error");
            return;
        }

        creatingClient = true;
        try {
            const created = await idpApi.register({
                clientRegistration: {
                    application: { uri: application.uri },
                    clientName: clientName.trim(),
                    profile,
                    clientType,
                    tokenEndpointAuthMethod,
                    allowedGrantTypes: defaultAllowedGrantTypes,
                    responseTypes: defaultResponseTypes,
                    redirectUris: parseLines(redirectUrisText),
                    allowedScopes: parseTokens(allowedScopesText),
                    clientUri: clientUri.trim() || undefined,
                },
            });
            resetClientForm();
            notifications.add("Client created", "success");
            clients = [...clients, created];
        } catch (cause) {
            console.error(cause);
            notifications.add("Failed to create client", "error");
        } finally {
            creatingClient = false;
        }
    }

    $effect(() => {
        selectedRoleId;
        if (selectedRoleId) {
            void loadRolePermissions();
        } else {
            rolePermissions = [];
        }
    });

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
                            bind:value={appUri}
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

        <Card title="Roles">
            <form class="flex flex-col gap-4" onsubmit={onCreateRole}>
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
                            bind:value={roleName}
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
                            bind:value={roleDescription}
                            rows="2"></textarea>
                    {/snippet}
                </Field>
                <div class="flex justify-end">
                    <button
                        class="btn btn-primary btn-md"
                        type="submit"
                        disabled={creatingRole}
                        >{creatingRole ? "Creating..." : "Create role"}</button
                    >
                </div>
            </form>

            {#if rolesError}
                <Alert tone="danger">{rolesError}</Alert>
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
                                    <td>{role.id}</td>
                                    <td>{role.name}</td>
                                    <td>{role.description ?? "-"}</td>
                                    <td>{formatTimestamp(role.updatedAt)}</td>
                                    <td>
                                        <button
                                            class="btn btn-danger btn-md"
                                            type="button"
                                            onclick={() => onDeleteRole(role)}
                                            disabled={deletingRoleId !== null}
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
            <form class="flex flex-col gap-4" onsubmit={onCreatePermission}>
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
                            bind:value={permissionName}
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
                            bind:value={permissionDescription}
                            rows="2"></textarea>
                    {/snippet}
                </Field>
                <div class="flex justify-end">
                    <button
                        class="btn btn-primary btn-md"
                        type="submit"
                        disabled={creatingPermission}
                        >{creatingPermission
                            ? "Creating..."
                            : "Create permission"}</button
                    >
                </div>
            </form>

            {#if permissionsError}
                <Alert tone="danger">{permissionsError}</Alert>
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
                                    <td>{permission.id}</td>
                                    <td>{permission.name}</td>
                                    <td>{permission.description ?? "-"}</td>
                                    <td>
                                        {formatTimestamp(permission.updatedAt)}
                                    </td>
                                    <td>
                                        <button
                                            class="btn btn-danger btn-md"
                                            type="button"
                                            onclick={() =>
                                                onDeletePermission(permission)}
                                            disabled={deletingPermissionId !==
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
            <div class="grid gap-3 md:grid-cols-3 md:items-end">
                <Field label="Role">
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
                            bind:value={selectedRoleId}
                        >
                            <option value="">Select role</option>
                            {#each roles as role (role.id)}
                                <option value={String(role.id)}
                                    >{role.name}</option
                                >
                            {/each}
                        </select>
                    {/snippet}
                </Field>
                <Field label="Permission">
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
                            bind:value={selectedPermissionId}
                        >
                            <option value="">Select permission</option>
                            {#each permissions as permission (permission.id)}
                                <option value={String(permission.id)}
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
                    disabled={!selectedRoleId ||
                        !selectedPermissionId ||
                        assigningPermission}
                    >{assigningPermission
                        ? "Assigning..."
                        : "Assign permission"}</button
                >
            </div>

            {#if rolePermissionsError}
                <Alert tone="danger">{rolePermissionsError}</Alert>
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
                                    <td>{permission.id}</td>
                                    <td>{permission.name}</td>
                                    <td>{permission.description ?? "-"}</td>
                                    <td>
                                        <button
                                            class="btn btn-danger btn-md"
                                            type="button"
                                            onclick={() =>
                                                onRevokePermissionFromRole(
                                                    permission.id,
                                                )}
                                            disabled={revokingRolePermissionId !==
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
        </Card>

        <Card title="Clients">
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
                    <Field label="Client type">
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
                                bind:value={clientType}
                            >
                                {#each clientTypeOptions as option (option)}
                                    <option value={option}>{option}</option>
                                {/each}
                            </select>
                        {/snippet}
                    </Field>
                    <Field label="Token endpoint auth method">
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
                                bind:value={tokenEndpointAuthMethod}
                            >
                                {#each tokenEndpointAuthMethodOptions as option (option)}
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

            {#if applicationClients.length === 0}
                <p>
                    Created clients appear here for this session. The IdP does
                    not expose a client directory. <a href={resolve("/clients")}
                        >Look up a client by ID</a
                    >.
                </p>
            {:else}
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
                            {#each applicationClients as client (client.clientId ?? `${client.clientName}-${client.application.uri}`)}
                                <tr>
                                    <td>
                                        {#if client.clientId}
                                            <a
                                                href={resolve(
                                                    `/clients/${client.clientId}`,
                                                )}>{client.clientId}</a
                                            >
                                        {:else}
                                            <span class="disabled">-</span>
                                        {/if}
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
