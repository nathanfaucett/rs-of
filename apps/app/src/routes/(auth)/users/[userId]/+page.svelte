<script lang="ts">
    import type { UserInfo } from "@aicacia/idp-client";
    import type { UserApplicationRoleResponse } from "@aicacia/management-client";
    import { onMount } from "svelte";
    import { page } from "$app/state";
    import {
        type Consent,
        idpAdministration,
    } from "$lib/common/state/idpAdministration";
    import { idpApi } from "$lib/common/state/idpClient.svelte";
    import { managementApi } from "$lib/common/state/managementClient.svelte";
    import { notifications } from "$lib/common/state/notifications.svelte";
    import { Alert, Card, Page, PageHeader } from "$lib/ui";

    const parsedUserId = page.params.userId;
    const hasValidUserId = Boolean(parsedUserId);

    let loading = $state(false);
    let error = $state<string | null>(null);
    let user = $state<UserInfo | null>(null);
    let userRolesLoading = $state(false);
    let userRolesError = $state<string | null>(null);
    let userRoles = $state<UserApplicationRoleResponse[]>([]);
    let consentsLoading = $state(false);
    let consentsError = $state<string | null>(null);
    let consents = $state<Consent[]>([]);
    let revokingConsentId = $state<string | null>(null);

    async function loadUserRoles() {
        if (!hasValidUserId) {
            return;
        }

        userRolesLoading = true;
        userRolesError = null;
        try {
            userRoles = await managementApi.listUserRolesAcrossApplications({
                userId: parsedUserId,
            });
        } catch (cause) {
            console.error(cause);
            userRolesError =
                cause instanceof Error
                    ? cause.message
                    : "Failed to load assigned roles";
            notifications.add("Failed to load assigned roles", "error");
        } finally {
            userRolesLoading = false;
        }
    }

    async function loadConsents() {
        if (!hasValidUserId) {
            return;
        }

        consentsLoading = true;
        consentsError = null;
        try {
            consents = await idpAdministration.listConsents({
                userId: parsedUserId,
                offset: 0,
                limit: 100,
            });
        } catch (cause) {
            console.error(cause);
            consentsError =
                cause instanceof Error
                    ? cause.message
                    : "Failed to load user consents";
            notifications.add("Failed to load user consents", "error");
        } finally {
            consentsLoading = false;
        }
    }

    async function loadRelatedData() {
        await Promise.all([loadUserRoles(), loadConsents()]);
    }

    async function loadUser() {
        if (!hasValidUserId) {
            error = "Invalid user ID";
            return;
        }

        loading = true;
        error = null;
        try {
            user = await idpApi.getUser({
                userId: parsedUserId,
            });
            void loadRelatedData();
        } catch (cause) {
            console.error(cause);
            error =
                cause instanceof Error ? cause.message : "Failed to load user";
            notifications.add("Failed to load user", "error");
        } finally {
            loading = false;
        }
    }

    async function onRevokeConsent(consent: Consent) {
        if (!hasValidUserId) {
            return;
        }

        if (!confirm(`Revoke consent for ${consent.client_id}?`)) {
            return;
        }

        revokingConsentId = consent.id;
        try {
            await idpApi.revokeConsent({
                userId: parsedUserId,
                consentId: consent.id,
            });
            notifications.add("Consent revoked", "success");
            await loadConsents();
        } catch (cause) {
            console.error(cause);
            notifications.add("Failed to revoke consent", "error");
        } finally {
            revokingConsentId = null;
        }
    }

    onMount(() => {
        void loadUser();
    });
</script>

<Page>
    <PageHeader title="User Detail" />

    {#if error}
        <Alert tone="danger">{error}</Alert>
    {:else if loading}
        <Card tone="subtle">Loading user...</Card>
    {:else if user}
        <Card title="Profile">
            <p class="mb-0"><strong>Sub:</strong> {user.sub}</p>
            <p class="mb-0"><strong>Name:</strong> {user.name ?? "-"}</p>
            <p class="mb-0">
                <strong>Given name:</strong>
                {user.givenName ?? "-"}
            </p>
            <p class="mb-0">
                <strong>Family name:</strong>
                {user.familyName ?? "-"}
            </p>
            <p class="mb-0"><strong>Email:</strong> {user.email ?? "-"}</p>
            <p class="mb-0">
                <strong>Locale:</strong>
                {user.locale ?? "-"}
            </p>
        </Card>

        <Card title="Roles across applications">
            {#if userRolesError}
                <Alert tone="danger">{userRolesError}</Alert>
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
                                    <td>{userRole.applicationId}</td>
                                    <td>{userRole.roleId}</td>
                                    <td>{userRole.roleName}</td>
                                    <td>{userRole.roleDescription ?? "-"}</td>
                                </tr>
                            {/each}
                        </tbody>
                    </table>
                </div>
            {/if}
        </Card>

        <Card title="Consents">
            {#if consentsError}
                <Alert tone="danger">{consentsError}</Alert>
            {:else if consentsLoading}
                <Card tone="subtle">Loading consents...</Card>
            {:else if consents.length === 0}
                <Card tone="subtle">No consents found.</Card>
            {:else}
                <div
                    class="overflow-x-auto rounded-card border border-line bg-surface"
                >
                    <table class="table">
                        <thead>
                            <tr>
                                <th>Client ID</th>
                                <th>Scope</th>
                                <th>Redirect URI</th>
                                <th>Actions</th>
                            </tr>
                        </thead>
                        <tbody>
                            {#each consents as consent (consent.id)}
                                <tr>
                                    <td>{consent.client_id}</td>
                                    <td>{consent.scope}</td>
                                    <td class="break-all"
                                        >{consent.redirect_uri}</td
                                    >
                                    <td>
                                        <button
                                            class="btn btn-danger btn-md"
                                            type="button"
                                            onclick={() =>
                                                onRevokeConsent(consent)}
                                            disabled={revokingConsentId !==
                                                null}
                                            >{revokingConsentId === consent.id
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
    {/if}
</Page>
