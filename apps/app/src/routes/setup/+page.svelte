<script lang="ts">
    import { refreshAll } from "$app/navigation";
    import { chooseNodeSetup } from "$lib/common/state/nodeSetup";
    import { Alert, Card, Field, Page, PageHeader } from "$lib/ui";

    import type { PageProps } from "./$types";

    let { data }: PageProps = $props();
    let code = $state("");
    let busy = $state(false);
    let error = $state("");

    $effect(() => {
        if (data.state !== "starting") return;
        const timer = setInterval(() => void refreshAll(), 500);
        return () => clearInterval(timer);
    });

    async function choose(choice: "create" | "join") {
        busy = true;
        error = "";
        try {
            await chooseNodeSetup(choice, code);
            code = "";
            await refreshAll();
        } catch (cause) {
            error = cause instanceof Error ? cause.message : "Cannot select Node setup.";
        } finally {
            busy = false;
        }
    }
</script>

<Page width="narrow">
    <PageHeader title="Set up this Node" />
    <Card>
        {#if error}<Alert tone="danger">{error}</Alert>{/if}
        {#if data.state === "starting"}
            <p>Your choice was saved. Starting the selected setup.</p>
        {:else if data.state === "unavailable"}
            <Alert tone="danger">Cannot verify Node setup. Check the server terminal and HTTPS trust.</Alert>
        {:else}
            <p>Use the protected Node setup code from the file shown in the server terminal.</p>
            <Field label="Node setup code">
                {#snippet children({ id })}
                    <input {id} type="password" bind:value={code} autocomplete="off" disabled={busy} class="input" />
                {/snippet}
            </Field>
            <div class="flex flex-wrap gap-4">
                <button type="button" class="btn btn-primary btn-md" disabled={busy} onclick={() => choose("create")}>Start a new Installation</button>
                <button type="button" class="btn btn-secondary btn-md" disabled={busy} onclick={() => choose("join")}>Join an existing Installation</button>
            </div>
        {/if}
        <button type="button" class="btn btn-secondary btn-md" disabled={busy} onclick={() => refreshAll()}>Check Node status</button>
    </Card>
</Page>
