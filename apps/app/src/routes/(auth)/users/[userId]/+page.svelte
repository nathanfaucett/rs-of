<script lang="ts">
    import type { UserInfo } from "@ofsdk/idp-client";
    import type { UserApplicationRoleResponse } from "@ofsdk/management-client";
    import { onMount } from "svelte";
    import { page } from "$app/state";
    import { resolve } from "$app/paths";
    import { goto } from "$app/navigation";
    import Modal from "$lib/common/components/Modal.svelte";
    import {
        type Consent,
        idpAdministration,
    } from "$lib/common/state/idpAdministration";
    import { idpApi } from "$lib/common/state/idpClient.svelte";
    import { managementApi } from "$lib/common/state/managementClient.svelte";
    import { notifications } from "$lib/common/state/notifications.svelte";
    import { Alert, Card, Page, PageHeader, Field } from "$lib/ui";

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

    // Edit mode
    let editing = $state(false);
    let editName = $state("");
    let editGivenName = $state("");
    let editFamilyName = $state("");
    let editEmail = $state("");
    let editPhoneNumber = $state("");
    let editLocale = $state("");
    let saving = $state(false);
    let saveError = $state<string | null>(null);

    // Delete confirmation
    let deleteModalRef: Modal;
    let deleting = $state(false);

    // Reset password
    let resetPasswordModalRef: Modal;
    let resettingPassword = $state(false);

    // Security: MFA and block
    let mfaTotpEnabled = $state(false);
    let mfaLoading = $state(false);
    let blockLoading = $state(false);

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
            if (user) {
                editName = user.name ?? "";
                editGivenName = user.givenName ?? "";
                editFamilyName = user.familyName ?? "";
                editEmail = user.email ?? "";
                editPhoneNumber = user.phoneNumber ?? "";
                editLocale = user.locale ?? "";
                mfaTotpEnabled = user.mfaTotpEnabled ?? false;
            }
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

    function startEdit() {
        editing = true;
        saveError = null;
    }

    function cancelEdit() {
        editing = false;
        saveError = null;
        if (user) {
            editName = user.name ?? "";
            editGivenName = user.givenName ?? "";
            editFamilyName = user.familyName ?? "";
            editEmail = user.email ?? "";
            editPhoneNumber = user.phoneNumber ?? "";
            editLocale = user.locale ?? "";
        }
    }

    async function saveEdit() {
        if (!hasValidUserId || !user) return;
        saving = true;
        saveError = null;
        try {
            const updates: Record<string, unknown> = {};
            if (editName !== (user.name ?? "")) updates.name = editName || null;
            if (editGivenName !== (user.givenName ?? ""))
                updates.givenName = editGivenName || null;
            if (editFamilyName !== (user.familyName ?? ""))
                updates.familyName = editFamilyName || null;
            if (editEmail !== (user.email ?? ""))
                updates.email = editEmail || null;
            if (editPhoneNumber !== (user.phoneNumber ?? ""))
                updates.phoneNumber = editPhoneNumber || null;
            if (editLocale !== (user.locale ?? ""))
                updates.locale = editLocale || null;

            if (Object.keys(updates).length > 0) {
                await idpApi.updateUser({
                    userId: parsedUserId,
                    updateUserInfoRequest: updates,
                });
                notifications.add("User updated", "success");
            }
            editing = false;
            await loadUser();
        } catch (cause) {
            console.error(cause);
            saveError =
                cause instanceof Error
                    ? cause.message
                    : "Failed to update user";
            notifications.add("Failed to update user", "error");
        } finally {
            saving = false;
        }
    }

    function confirmDelete() {
        deleteModalRef?.show();
    }

    async function deleteUser() {
        if (!hasValidUserId || !user) return;
        deleting = true;
        try {
            await idpApi.deleteUser({ userId: parsedUserId });
            notifications.add("User deleted", "success");
            deleteModalRef?.close();
            await goto("/users");
        } catch (cause) {
            console.error(cause);
            notifications.add("Failed to delete user", "error");
        } finally {
            deleting = false;
        }
    }

    function confirmResetPassword() {
        resetPasswordModalRef?.show();
    }

    async function resetPassword() {
        if (!hasValidUserId || !user) return;
        resettingPassword = true;
        try {
            await idpApi.resetPassword({
                userId: parsedUserId,
                resetPassword: { password: "" }, // Server will generate a reset link/email
            });
            notifications.add("Password reset email sent", "success");
            resetPasswordModalRef?.close();
        } catch (cause) {
            console.error(cause);
            notifications.add("Failed to send password reset", "error");
        } finally {
            resettingPassword = false;
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

    async function onToggleMfa() {
        if (!hasValidUserId) return;
        mfaLoading = true;
        try {
            await idpApi.setUserMfa({
                userId: parsedUserId,
                setUserMfaRequest: {
                    method: "totp",
                    enable: !mfaTotpEnabled,
                },
            });
            mfaTotpEnabled = !mfaTotpEnabled;
            notifications.add(
                mfaTotpEnabled ? "TOTP MFA enabled" : "TOTP MFA disabled",
                "success",
            );
        } catch (cause) {
            console.error(cause);
            notifications.add("Failed to update MFA", "error");
        } finally {
            mfaLoading = false;
        }
    }

    async function onToggleBlock() {
        if (!hasValidUserId || !user) return;
        blockLoading = true;
        try {
            const blocked = !isBlocked;
            await idpApi.setUserBlocked({
                userId: parsedUserId,
                setUserBlockedRequest: { blocked },
            });
            notifications.add(
                blocked ? "User blocked" : "User unblocked",
                "success",
            );
            await loadUser();
        } catch (cause) {
            console.error(cause);
            notifications.add("Failed to update block status", "error");
        } finally {
            blockLoading = false;
        }
    }

    const isBlocked = $derived(user?.blocked ?? false);

    onMount(() => {
        void loadUser();
    });
</script>

<Page>
    <PageHeader title="User Detail">
        {#snippet actions()}
            <div class="flex items-center gap-2">
                {#if user && !editing}
                    <button
                        class="btn btn-secondary btn-md"
                        type="button"
                        onclick={startEdit}
                    >
                        Edit
                    </button>
                    <button
                        class="btn btn-secondary btn-md"
                        type="button"
                        onclick={confirmResetPassword}
                    >
                        Reset password
                    </button>
                    <button
                        class="btn btn-danger btn-md"
                        type="button"
                        onclick={confirmDelete}
                    >
                        Delete
                    </button>
                {/if}
            </div>
        {/snippet}
    </PageHeader>

    {#if error}
        <Alert tone="danger">{error}</Alert>
    {:else if loading}
        <Card tone="subtle">Loading user...</Card>
    {:else if user}
        <Card title="Profile">
            {#if editing}
                <form
                    class="flex flex-col gap-4"
                    onsubmit={(e) => {
                        e.preventDefault();
                        void saveEdit();
                    }}
                >
                    <Field label="Name">
                        {#snippet children(control)}
                            <input
                                bind:value={editName}
                                type="text"
                                {...control}
                            />
                        {/snippet}
                    </Field>
                    <Field label="Given name">
                        {#snippet children(control)}
                            <input
                                bind:value={editGivenName}
                                type="text"
                                {...control}
                            />
                        {/snippet}
                    </Field>
                    <Field label="Family name">
                        {#snippet children(control)}
                            <input
                                bind:value={editFamilyName}
                                type="text"
                                {...control}
                            />
                        {/snippet}
                    </Field>
                    <Field label="Email">
                        {#snippet children(control)}
                            <input
                                bind:value={editEmail}
                                type="email"
                                {...control}
                            />
                        {/snippet}
                    </Field>
                    <Field label="Phone number">
                        {#snippet children(control)}
                            <input
                                bind:value={editPhoneNumber}
                                type="tel"
                                {...control}
                            />
                        {/snippet}
                    </Field>
                    <Field label="Locale">
                        {#snippet children(control)}
                            <input
                                bind:value={editLocale}
                                type="text"
                                {...control}
                            />
                        {/snippet}
                    </Field>
                    {#if saveError}
                        <Alert tone="danger">{saveError}</Alert>
                    {/if}
                    <div class="flex justify-end gap-2">
                        <button
                            class="btn btn-secondary btn-md"
                            type="button"
                            onclick={cancelEdit}
                            disabled={saving}
                        >
                            Cancel
                        </button>
                        <button
                            class="btn btn-primary btn-md"
                            type="submit"
                            disabled={saving}
                        >
                            {saving ? "Saving..." : "Save"}
                        </button>
                    </div>
                </form>
            {:else}
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
                    <strong>Phone:</strong>
                    {user.phoneNumber ?? "-"}
                </p>
                <p class="mb-0">
                    <strong>Locale:</strong>
                    {user.locale ?? "-"}
                </p>
            {/if}
        </Card>

        <Card title="Security">
            <div class="flex flex-col gap-4">
                <div class="flex items-center justify-between">
                    <div>
                        <p class="mb-0 font-medium">TOTP Multi-factor Auth</p>
                        <p class="mb-0 text-sm text-ink-muted">
                            {mfaTotpEnabled ? "Enabled" : "Disabled"}
                        </p>
                    </div>
                    <button
                        class="btn {mfaTotpEnabled
                            ? 'btn-secondary'
                            : 'btn-primary'} btn-md"
                        type="button"
                        onclick={onToggleMfa}
                        disabled={mfaLoading}
                    >
                        {mfaLoading
                            ? "Updating..."
                            : mfaTotpEnabled
                              ? "Disable"
                              : "Enable"}
                    </button>
                </div>
                <div class="flex items-center justify-between">
                    <div>
                        <p class="mb-0 font-medium">Account Status</p>
                        <p class="mb-0 text-sm text-ink-muted">
                            {isBlocked ? "Blocked" : "Active"}
                        </p>
                    </div>
                    <button
                        class="btn {isBlocked
                            ? 'btn-primary'
                            : 'btn-danger'} btn-md"
                        type="button"
                        onclick={onToggleBlock}
                        disabled={blockLoading}
                    >
                        {blockLoading
                            ? "Updating..."
                            : isBlocked
                              ? "Unblock"
                              : "Block"}
                    </button>
                </div>
            </div>
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

    <Modal bind:this={deleteModalRef} title="Delete User">
        <div class="flex flex-col gap-4">
            <p class="mb-0">
                Are you sure you want to delete user <strong>{user?.sub}</strong
                >? This action cannot be undone.
            </p>
            <form method="dialog" class="flex justify-end gap-2">
                <button
                    class="btn btn-secondary btn-md"
                    type="button"
                    formmethod="dialog"
                >
                    Cancel
                </button>
                <button
                    class="btn btn-danger btn-md"
                    type="submit"
                    onclick={deleteUser}
                    disabled={deleting}
                >
                    {deleting ? "Deleting..." : "Delete"}
                </button>
            </form>
        </div>
    </Modal>

    <Modal bind:this={resetPasswordModalRef} title="Reset Password">
        <div class="flex flex-col gap-4">
            <p class="mb-0">
                This will send a password reset email to <strong
                    >{user?.email ?? "the user's email"}</strong
                >.
            </p>
            <form method="dialog" class="flex justify-end gap-2">
                <button
                    class="btn btn-secondary btn-md"
                    type="button"
                    formmethod="dialog"
                >
                    Cancel
                </button>
                <button
                    class="btn btn-primary btn-md"
                    type="submit"
                    onclick={resetPassword}
                    disabled={resettingPassword}
                >
                    {resettingPassword ? "Sending..." : "Send reset email"}
                </button>
            </form>
        </div>
    </Modal>
</Page>
