<script lang="ts">
    import { goto } from "$app/navigation";
    import { resolve } from "$app/paths";
    import ConfigForm from "$lib/common/components/ConfigForm.svelte";
    import { m } from "$lib/paraglide/messages";
    import { Card, Page, PageHeader } from "$lib/ui";
</script>

<Page width="narrow">
    <PageHeader title={m.env_config()} />
    <Card>
        <ConfigForm onSaved={() => void goto(resolve("/"))} />
    </Card>
</Page>
