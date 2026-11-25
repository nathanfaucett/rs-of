<script lang="ts">
    import { ResponseError, type UserInfo } from "@ofsdk/idp-client";
    import { idpApi } from "$lib/common/state/idpClient.svelte";
    import { Alert, Card } from "$lib/ui";

    let { user }: { user: UserInfo } = $props();

    type Action = "mfa" | "block";
    let pending = $state<Action | null>(null);
    let result = $state<{ tone: "success" | "danger"; message: string } | null>(null);
    let mfaEnabled = $derived(user.mfaTotpEnabled ?? false);
    let blocked = $derived(user.blocked ?? false);

    function failure(action: Action, cause: unknown): string {
        if (cause instanceof ResponseError) {
            switch (cause.response.status) {
                case 401:
                    return "Sign in again to change this account.";
                case 403:
                    return `Permission denied. Ask an Installation administrator for idp.users.${action}.`;
                case 404:
                    return "This User was not found. Return to Users and select an existing account.";
                default:
                    return "The IdP did not confirm the change. Reload this User before retrying.";
            }
        }
        return "Could not confirm the change. Check the connection and reload this User before retrying.";
    }

    async function update(action: Action, enable: boolean) {
        if (pending) return;
        const userId = user.sub;
        pending = action;
        result = null;
        try {
            if (action === "mfa") {
                await idpApi.setUserMfa({
                    userId,
                    setUserMfaRequest: { method: "totp", enable },
                });
                mfaEnabled = enable;
                result = { tone: "success", message: enable ? "TOTP MFA enabled." : "TOTP MFA disabled." };
            } else {
                await idpApi.setUserBlocked({
                    userId,
                    setUserBlockedRequest: { blocked: enable },
                });
                blocked = enable;
                result = { tone: "success", message: enable ? "User blocked." : "User unblocked." };
            }
        } catch (cause) {
            result = { tone: "danger", message: failure(action, cause) };
        } finally {
            pending = null;
        }
    }

    function changeBlock() {
        const label = user.name || user.email;
        const target = label ? `${label} (${user.sub})` : user.sub;
        if (!blocked && !confirm(`Block ${target}? This prevents new sign-ins.`)) return;
        void update("block", !blocked);
    }
</script>

<Card title="Security">
    <div class="flex flex-col gap-4">
        <div class="flex items-center justify-between">
            <div>
                <p class="mb-0 font-medium">TOTP Multi-factor Auth</p>
                <p class="mb-0 text-sm text-ink-muted">
                    {mfaEnabled ? "Enabled" : "Disabled"}
                </p>
            </div>
            <button
                class="btn {mfaEnabled ? 'btn-secondary' : 'btn-primary'} btn-md"
                type="button"
                onclick={() => update("mfa", !mfaEnabled)}
                disabled={pending !== null}
            >
                {pending === "mfa" ? "Updating..." : mfaEnabled ? "Disable" : "Enable"}
            </button>
        </div>
        <div class="flex items-center justify-between">
            <div>
                <p class="mb-0 font-medium">Account Status</p>
                <p class="mb-0 text-sm text-ink-muted">
                    {blocked ? "Blocked" : "Active"}
                </p>
            </div>
            <button
                class="btn {blocked ? 'btn-primary' : 'btn-danger'} btn-md"
                type="button"
                onclick={changeBlock}
                disabled={pending !== null}
            >
                {pending === "block" ? "Updating..." : blocked ? "Unblock" : "Block"}
            </button>
        </div>
        {#if result}
            <Alert tone={result.tone}>{result.message}</Alert>
        {/if}
    </div>
</Card>
