<script lang="ts">
    import type {
        RoleResponse,
        UserApplicationRoleResponse,
    } from "@ofsdk/management-client";
    import { onMount } from "svelte";
    import {
        type Application,
        idpAdministration,
    } from "$lib/common/state/idpAdministration";
    import { managementApi } from "$lib/common/state/managementClient.svelte";
    import {
        managementPolicyError,
        managementPolicyStatus,
    } from "$lib/common/state/managementPolicyError";
    import { notifications } from "$lib/common/state/notifications.svelte";
    import { Alert, Card, Field } from "$lib/ui";

    let { userId }: { userId: string } = $props();
    let userRolesLoading = $state(true);
    let userRolesError = $state<{ text: string; retry: boolean } | null>(null);
    let userRoles = $state.raw<UserApplicationRoleResponse[]>([]);
    let applicationsLoading = $state(true);
    let applicationsError = $state<{ text: string; retry: boolean } | null>(
        null,
    );
    let applications = $state.raw<Application[]>([]);
    let selectedApplicationId = $state("");
    let rolesLoading = $state(false);
    let rolesError = $state<{ text: string; retry: boolean } | null>(null);
    let roles = $state.raw<RoleResponse[]>([]);
    let selectedRoleId = $state("");
    let assigning = $state(false);
    let revokingRoleId = $state<string | null>(null);
    let writeError = $state<string | null>(null);
    let availableRoles = $derived(
        roles.filter(
            (role) =>
                !userRoles.some(
                    (userRole) =>
                        userRole.applicationId === selectedApplicationId &&
                        userRole.roleId === role.id,
                ),
        ),
    );

    function syncRoleSelection() {
        if (!availableRoles.some((role) => role.id === selectedRoleId)) {
            selectedRoleId = availableRoles[0]?.id ?? "";
        }
    }

    async function load() {
        userRolesLoading = true;
        userRolesError = null;
        try {
            userRoles = await managementApi.listUserRolesAcrossApplications({
                userId,
            });
            syncRoleSelection();
        } catch (cause) {
            userRolesError = {
                text: managementPolicyError(cause, "view assigned roles"),
                retry: managementPolicyStatus(cause) !== 403,
            };
        } finally {
            userRolesLoading = false;
        }
    }

    async function loadRoles() {
        const applicationId = selectedApplicationId;
        rolesLoading = true;
        rolesError = null;
        roles = [];
        selectedRoleId = "";
        if (!applicationId) {
            rolesLoading = false;
            return;
        }
        try {
            const result = await managementApi.listRoles({
                applicationId,
                offset: 0,
                limit: 100,
            });
            if (applicationId !== selectedApplicationId) return;
            roles = result;
            syncRoleSelection();
        } catch (cause) {
            if (applicationId !== selectedApplicationId) return;
            rolesError = {
                text: managementPolicyError(cause, "list roles"),
                retry: managementPolicyStatus(cause) !== 403,
            };
        } finally {
            if (applicationId === selectedApplicationId) rolesLoading = false;
        }
    }

    async function loadApplications() {
        applicationsLoading = true;
        applicationsError = null;
        try {
            applications = await idpAdministration.listApplications({
                offset: 0,
                limit: 100,
            });
            if (
                !applications.some(
                    (application) => application.id === selectedApplicationId,
                )
            ) {
                selectedApplicationId = applications[0]?.id ?? "";
            }
            await loadRoles();
        } catch (cause) {
            applicationsError = {
                text: managementPolicyError(cause, "list applications"),
                retry: managementPolicyStatus(cause) !== 403,
            };
        } finally {
            applicationsLoading = false;
        }
    }

    async function onAssignRole() {
        const applicationId = selectedApplicationId;
        const roleId = selectedRoleId;
        if (!applicationId || !roleId) {
            notifications.add("Select an application and role", "error");
            return;
        }
        writeError = null;
        assigning = true;
        try {
            await managementApi.assignRoleToUser({
                applicationId,
                userId,
                roleId,
            });
            notifications.add("Role assigned", "success");
            await load();
        } catch (cause) {
            writeError = managementPolicyError(cause, "assign a role");
        } finally {
            assigning = false;
        }
    }

    async function onRevokeRole(userRole: UserApplicationRoleResponse) {
        if (!confirm(`Revoke role "${userRole.roleName}"?`)) return;
        writeError = null;
        revokingRoleId = userRole.roleId;
        try {
            await managementApi.revokeRoleFromUser({
                applicationId: userRole.applicationId,
                userId,
                roleId: userRole.roleId,
            });
            notifications.add("Role revoked", "success");
            await load();
        } catch (cause) {
            writeError = managementPolicyError(cause, "revoke a role");
        } finally {
            revokingRoleId = null;
        }
    }

    onMount(() => {
        // Do not issue concurrent IdP token exchanges for the initial reads.
        void load().then(loadApplications);
    });
</script>

<Card
    title="Assigned roles"
    description="Assign or revoke Application roles for this User."
>
    {#if applicationsError}
        <Alert tone="danger">{applicationsError.text}</Alert>
        {#if applicationsError.retry}
            <button
                class="btn btn-secondary btn-md"
                type="button"
                onclick={loadApplications}
            >
                Retry
            </button>
        {/if}
    {:else}
        <div class="grid gap-3 md:grid-cols-3 md:items-end">
            <Field label="Application">
                {#snippet children(control)}
                    <select
                        {...control}
                        value={selectedApplicationId}
                        disabled={applicationsLoading ||
                            assigning ||
                            revokingRoleId !== null}
                        onchange={(event) => {
                            selectedApplicationId = event.currentTarget.value;
                            void loadRoles();
                        }}
                    >
                        <option value="">Select application</option>
                        {#each applications as application (application.id)}
                            <option value={application.id}>
                                {application.name}
                            </option>
                        {/each}
                    </select>
                {/snippet}
            </Field>
            <Field label="Role">
                {#snippet children(control)}
                    <select
                        {...control}
                        bind:value={selectedRoleId}
                        disabled={rolesLoading ||
                            assigning ||
                            revokingRoleId !== null}
                    >
                        <option value="">Select role</option>
                        {#each availableRoles as role (role.id)}
                            <option value={role.id}>{role.name}</option>
                        {/each}
                    </select>
                {/snippet}
            </Field>
            <button
                class="btn btn-primary btn-md"
                type="button"
                onclick={onAssignRole}
                disabled={!selectedApplicationId ||
                    !selectedRoleId ||
                    rolesLoading ||
                    assigning ||
                    revokingRoleId !== null}
            >
                {assigning ? "Assigning..." : "Assign role"}
            </button>
        </div>
        {#if rolesError}
            <Alert tone="danger">{rolesError.text}</Alert>
            {#if rolesError.retry}
                <button
                    class="btn btn-secondary btn-md"
                    type="button"
                    onclick={loadRoles}
                >
                    Retry
                </button>
            {/if}
        {/if}
    {/if}
    {#if writeError}<Alert tone="danger">{writeError}</Alert>{/if}

    {#if userRolesError}
        <Alert tone="danger">{userRolesError.text}</Alert>
        {#if userRolesError.retry}
            <button
                class="btn btn-secondary btn-md"
                type="button"
                onclick={load}
            >
                Retry
            </button>
        {/if}
    {:else if userRolesLoading}
        <Card tone="subtle">Loading roles...</Card>
    {:else if userRoles.length === 0}
        <Card tone="subtle">No roles assigned.</Card>
    {:else}
        <div class="overflow-x-auto rounded-card border border-line bg-surface">
            <table class="table">
                <thead>
                    <tr>
                        <th>Application</th>
                        <th>Role ID</th>
                        <th>Name</th>
                        <th>Description</th>
                        <th>Actions</th>
                    </tr>
                </thead>
                <tbody>
                    {#each userRoles as userRole (`${userRole.applicationId}-${userRole.roleId}`)}
                        <tr>
                            <td data-label="Application">
                                {userRole.applicationId}
                            </td>
                            <td data-label="Role ID">{userRole.roleId}</td>
                            <td data-label="Name">{userRole.roleName}</td>
                            <td data-label="Description">
                                {userRole.roleDescription ?? "-"}
                            </td>
                            <td data-label="Actions">
                                <button
                                    class="btn btn-danger btn-md"
                                    type="button"
                                    onclick={() => onRevokeRole(userRole)}
                                    disabled={assigning ||
                                        revokingRoleId !== null}
                                >
                                    {revokingRoleId === userRole.roleId
                                        ? "Revoking..."
                                        : "Revoke"}
                                </button>
                            </td>
                        </tr>
                    {/each}
                </tbody>
            </table>
        </div>
    {/if}
</Card>
