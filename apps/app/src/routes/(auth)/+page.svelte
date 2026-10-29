<script lang="ts">
    import { resolve } from "$app/paths";
    import { Card, Page, PageHeader } from "$lib/ui";
    import type { PageProps } from "./$types";

    let { data }: PageProps = $props();

    const userInfo = $derived(data.userInfo);
    const displayName = $derived(
        userInfo?.name ||
            userInfo?.preferredUsername ||
            userInfo?.givenName ||
            userInfo?.email ||
            "User",
    );
</script>

<Page>
    <PageHeader
        title="Dashboard"
        description="Manage applications, devices, and user records."
    />

    <Card>
        <p class="mb-0">Signed in as <strong>{displayName}</strong></p>
    </Card>

    <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
        <a
            class="rounded-card border border-line bg-surface p-5 no-underline shadow-sm transition-colors hover:bg-surface-hover"
            href={resolve("/applications")}
        >
            <h2 class="mb-1 text-lg">Applications</h2>
            <p class="mb-0 text-sm text-ink-muted">
                Manage application settings, roles, permissions, and clients.
            </p>
        </a>
        <a
            class="rounded-card border border-line bg-surface p-5 no-underline shadow-sm transition-colors hover:bg-surface-hover"
            href={resolve("/devices")}
        >
            <h2 class="mb-1 text-lg">Devices</h2>
            <p class="mb-0 text-sm text-ink-muted">
                Approve and revoke the devices that can synchronize your files.
            </p>
        </a>
        <a
            class="rounded-card border border-line bg-surface p-5 no-underline shadow-sm transition-colors hover:bg-surface-hover"
            href={resolve("/users")}
        >
            <h2 class="mb-1 text-lg">Users</h2>
            <p class="mb-0 text-sm text-ink-muted">
                Browse and manage user records.
            </p>
        </a>
    </div>
</Page>
