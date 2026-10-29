<script lang="ts">
    import { goto } from "$app/navigation";
    import { resolve } from "$app/paths";
    import { Card, Field, Page, PageHeader } from "$lib/ui";

    let clientId = $state("");

    function openClient(event: SubmitEvent) {
        event.preventDefault();
        if (clientId.trim())
            void goto(
                resolve(`/clients/${encodeURIComponent(clientId.trim())}`),
            );
    }
</script>

<Page>
    <PageHeader
        title="Clients"
        description="Create clients from an application. The IdP supports authorized lookup by client ID, not a client directory."
    />

    <Card>
        <form class="flex flex-col gap-4" onsubmit={openClient}>
            <Field label="Client ID">
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
                        bind:value={clientId}
                    />
                {/snippet}
            </Field>
            <div class="flex justify-end">
                <button class="btn btn-primary btn-md" type="submit"
                    >Open client</button
                >
            </div>
        </form>
    </Card>
</Page>
