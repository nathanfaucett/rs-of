<script lang="ts">
    import { resolve } from "$app/paths";
    import { Card, Page, PageHeader } from "$lib/ui";
</script>

<Page>
    <PageHeader
        title="Roles"
        description="Select an application to create, update, or remove roles."
    />

    <Card title="Roles are managed per application">
        <a class="btn btn-primary btn-md" href={resolve("/applications")}
            >Go to applications</a
        >
    </Card>
</Page>
