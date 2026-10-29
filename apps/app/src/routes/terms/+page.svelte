<script lang="ts">
    import { m } from "$lib/paraglide/messages";
    import { Card, Page, PageHeader } from "$lib/ui";
</script>

<Page>
    <PageHeader title={m.terms_title()} description={m.terms_intro()} />
    <Card title={m.terms_title()}>
        <ul class="list-disc space-y-2 ps-5 text-sm">
            <li>{m.terms_use()}</li>
            <li>{m.terms_liability()}</li>
            <li>{m.terms_changes()}</li>
        </ul>
    </Card>
</Page>
