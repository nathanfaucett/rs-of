<script lang="ts">
    import { handleNativeFetchCallback } from "@aicacia/native-fetch";
    import { onMount } from "svelte";
    import { page } from "$app/state";
    import { Alert, Card, Page, PageHeader } from "$lib/ui";

    let callbackError = $state("");

    onMount(() => {
        try {
            handleNativeFetchCallback(page.url.searchParams);
        } catch (err) {
            callbackError = err instanceof Error ? err.message : String(err);
        }
    });
</script>

<Page>
    <PageHeader title="Native callback" />
    {#if callbackError}
        <Card title="Native Callback failed">
            <Alert tone="danger">{callbackError}</Alert>
        </Card>
    {:else}
        <Card tone="subtle">
            <p>Processing native callback...</p>
        </Card>
    {/if}
</Page>
