<script lang="ts" module>
    export type CardTone = "default" | "subtle" | "danger";

    export interface CardProps {
        title?: string;
        description?: string;
        tone?: CardTone;
        actions?: import("svelte").Snippet;
        children: import("svelte").Snippet;
    }
</script>

<script lang="ts">
    let {
        title,
        description,
        tone = "default",
        actions,
        children,
    }: CardProps = $props();

    const headingId = $props.id();
</script>

<section class="card {tone}" aria-labelledby={title ? headingId : undefined}>
    {#if title || description || actions}
        <header class="card-header">
            <div class="card-heading">
                {#if title}<h2 id={headingId}>{title}</h2>{/if}
                {#if description}<p class="card-description">{description}</p>{/if}
            </div>
            {#if actions}<div class="card-actions">{@render actions()}</div>{/if}
        </header>
    {/if}
    <div class="card-body">
        {@render children()}
    </div>
</section>

<style>
    .card {
        border: 1px solid var(--border);
        border-radius: var(--radius-card);
        background-color: var(--surface);
        box-shadow: var(--shadow);
    }

    .card.subtle {
        background-color: var(--surface-muted);
        box-shadow: none;
    }

    .card.danger {
        border-color: var(--danger);
        background-color: var(--danger-soft);
    }

    .card-header {
        display: flex;
        align-items: flex-start;
        justify-content: space-between;
        gap: 1rem;
        padding: 1rem 1.25rem;
        border-bottom: 1px solid var(--border);
    }

    .card.danger .card-header {
        border-bottom-color: color-mix(in oklab, var(--danger) 30%, transparent);
    }

    .card-heading {
        display: flex;
        flex-direction: column;
        gap: 0.125rem;
        min-width: 0;
    }

    .card-heading h2 {
        margin: 0;
        font-size: 0.9375rem;
        font-weight: 600;
        color: var(--text);
    }

    .card-description {
        margin: 0;
        font-size: 0.8125rem;
        color: var(--text-muted);
    }

    .card-actions {
        display: flex;
        flex-shrink: 0;
        align-items: center;
        gap: 0.5rem;
    }

    .card-body {
        padding: 1.25rem;
    }
</style>
