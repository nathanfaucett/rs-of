<script lang="ts" module>
    export interface PageHeaderProps {
        title: string;
        description?: string;
        actions?: import("svelte").Snippet;
    }
</script>

<script lang="ts">
    let { title, description, actions }: PageHeaderProps = $props();
</script>

<header class="page-header">
    <div class="page-heading">
        <h1>{title}</h1>
        {#if description}<p class="page-description">{description}</p>{/if}
    </div>
    {#if actions}<div class="page-actions">{@render actions()}</div>{/if}
</header>

<style>
    .page-header {
        display: flex;
        align-items: flex-start;
        justify-content: space-between;
        gap: 1rem;
        flex-wrap: wrap;
    }

    .page-heading {
        min-width: 0;
    }

    .page-heading h1 {
        margin: 0;
        font-size: 1.25rem;
        font-weight: 600;
        color: var(--text);
    }

    .page-description {
        margin: 0.25rem 0 0;
        font-size: 0.875rem;
        color: var(--text-muted);
    }

    .page-actions {
        display: flex;
        align-items: center;
        gap: 0.5rem;
    }
</style>
