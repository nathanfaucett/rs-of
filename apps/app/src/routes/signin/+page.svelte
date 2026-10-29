<script lang="ts">
    import { m } from "$lib/paraglide/messages";
    import { Card, Page, PageHeader } from "$lib/ui";
    import SignIn from "./_SignIn.svelte";
</script>

<Page width="narrow">
    <PageHeader title={m.sign_in()} />
    <Card>
        <SignIn />
    </Card>
</Page>
