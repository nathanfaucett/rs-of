<script lang="ts">
    import type { UserApplicationRoleResponse } from "@ofsdk/management-client";
    import { onMount } from "svelte";
    import { managementApi } from "$lib/common/state/managementClient.svelte";
    import { managementPolicyError, managementPolicyStatus } from "$lib/common/state/managementPolicyError";
    import { Alert, Card } from "$lib/ui";

    let { userId }: { userId: string } = $props();
    let userRolesLoading = $state(true);
    let userRolesError = $state<{ text: string; retry: boolean } | null>(null);
    let userRoles = $state.raw<UserApplicationRoleResponse[]>([]);

    async function load() {
        userRolesLoading = true;
        userRolesError = null;
        try {
            userRoles = await managementApi.listUserRolesAcrossApplications({ userId });
        } catch (cause) {
            userRolesError = {
                text: managementPolicyError(cause, "view assigned roles"),
                retry: managementPolicyStatus(cause) !== 403,
            };
        } finally {
            userRolesLoading = false;
        }
    }

    onMount(() => { void load(); });
</script>

<Card title="Assigned roles" description="Shows the Application roles you have permission to view.">
    {#if userRolesError}
        <Alert tone="danger">{userRolesError.text}</Alert>
        {#if userRolesError.retry}
            <button class="btn btn-secondary btn-md" type="button" onclick={load}>Retry</button>
        {/if}
    {:else if userRolesLoading}
        <Card tone="subtle">Loading roles...</Card>
    {:else if userRoles.length === 0}
        <Card tone="subtle">No roles assigned.</Card>
    {:else}
        <div
            class="overflow-x-auto rounded-card border border-line bg-surface"
        >
            <table class="table">
                <thead>
                    <tr>
                        <th>Application</th>
                        <th>Role ID</th>
                        <th>Name</th>
                        <th>Description</th>
                    </tr>
                </thead>
                <tbody>
                    {#each userRoles as userRole (`${userRole.applicationId}-${userRole.roleId}`)}
                        <tr>
                            <td data-label="Application">{userRole.applicationId}</td>
                            <td data-label="Role ID">{userRole.roleId}</td>
                            <td data-label="Name">{userRole.roleName}</td>
                            <td data-label="Description">{userRole.roleDescription ?? "-"}</td>
                        </tr>
                    {/each}
                </tbody>
            </table>
        </div>
    {/if}
</Card>
