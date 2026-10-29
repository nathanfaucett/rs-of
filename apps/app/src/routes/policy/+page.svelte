<script lang="ts">
    import { m } from "$lib/paraglide/messages";
    import { Card, Page, PageHeader } from "$lib/ui";
</script>

<Page>
    <PageHeader title={m.policy_title()} description={m.policy_summary()} />
    <Card title={m.policy_title()}>
        <ul class="list-disc space-y-2 ps-5 text-sm">
            <li>{m.policy_data()}</li>
            <li>{m.policy_use()}</li>
            <li>{m.policy_sharing()}</li>
            <li>{m.policy_retention()}</li>
        </ul>
    </Card>
</Page>
