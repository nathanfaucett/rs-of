<script lang="ts">
    import { goto } from "$app/navigation";
    import { resolve } from "$app/paths";
    import { page } from "$app/state";
    import { Alert, Page, PageHeader } from "$lib/ui";

    let retrying = $state(false);
    let retryError = $state("");

    async function retry() {
        retrying = true;
        retryError = "";
        try {
            // Reloading the document would discard the desktop's memory-only session.
            await goto(
                resolve(
                    (page.url.pathname +
                        page.url.search +
                        page.url.hash) as "/",
                ),
                {
                    invalidateAll: true,
                    replaceState: true,
                },
            );
        } catch {
            retryError = "Retry failed. Please try again.";
        } finally {
            retrying = false;
        }
    }
</script>

<svelte:head>
    <title>{page.status} — {page.error?.message ?? "Error"}</title>
</svelte:head>

<Page>
    <PageHeader title={String(page.status)} />
    <Alert tone="danger"
        >{page.error?.message ?? "An unexpected error occurred."}</Alert
    >
    {#if page.status === 502 || page.status === 503}
        <button
            class="btn btn-primary btn-md"
            type="button"
            onclick={retry}
            disabled={retrying}>{retrying ? "Retrying…" : "Retry"}</button
        >
        {#if retryError}
            <Alert tone="danger">{retryError}</Alert>
        {/if}
    {/if}
</Page>
