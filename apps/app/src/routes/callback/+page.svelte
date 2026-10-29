<script lang="ts">
    import { onMount } from "svelte";
    import { goto } from "$app/navigation";
    import { resolve } from "$app/paths";
    import { page } from "$app/state";
    import { getOidcClient } from "$lib/common/state/oidc.svelte";
    import { Alert, Card, Page, PageHeader } from "$lib/ui";

    let callbackError = $state("");
    let callbackErrorDescription = $state("");
    let callbackComplete = $state(false);
    let popupCallback = $state(false);

    onMount(async () => {
        const searchParams = new URLSearchParams(page.url.searchParams);
        const hashParams = new URLSearchParams(page.url.hash.replace(/^#/, ""));

        const error = searchParams.get("error") ?? hashParams.get("error");
        const errorDescription =
            searchParams.get("error_description") ??
            hashParams.get("error_description") ??
            "";

        if (error) {
            callbackError = error;
            callbackErrorDescription = errorDescription;
            return;
        }

        try {
            await getOidcClient().handleSigninCallback();
            popupCallback = window.opener != null && window.opener !== window;
            if (popupCallback) {
                callbackComplete = true;
                return;
            }
            await goto(resolve("/"));
        } catch (err) {
            callbackError = "callback_error";
            callbackErrorDescription =
                err instanceof Error ? err.message : String(err);
        }
    });
</script>

<Page>
    <PageHeader title="Callback" />
    {#if callbackError}
        <Card title="Callback failed">
            <Alert tone="danger">{callbackError}</Alert>
            {#if callbackErrorDescription}
                <p class="mt-2 text-sm break-words text-ink-muted">
                    {callbackErrorDescription}
                </p>
            {/if}
        </Card>
    {:else if callbackComplete && popupCallback}
        <Card title="Sign in complete" tone="subtle">
            <p>You can close this window and return to the app.</p>
        </Card>
    {:else}
        <Card tone="subtle">
            <p>Processing callback...</p>
        </Card>
    {/if}
</Page>
