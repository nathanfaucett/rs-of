<script lang="ts" module>
export type AlertTone = "info" | "success" | "warning" | "danger";

export interface AlertProps {
    tone?: AlertTone;
    title?: string;
    children: import("svelte").Snippet;
}
</script>

<script lang="ts">
    import CircleCheck from "@lucide/svelte/icons/circle-check";
    import CircleX from "@lucide/svelte/icons/circle-x";
    import Info from "@lucide/svelte/icons/info";
    import TriangleAlert from "@lucide/svelte/icons/triangle-alert";

    let { tone = "info", title, children }: AlertProps = $props();

    const icons = {
        info: Info,
        success: CircleCheck,
        warning: TriangleAlert,
        danger: CircleX,
    };
    const Icon = $derived(icons[tone]);
</script>

<div class="alert {tone}" role={tone === "danger" ? "alert" : "status"}>
    <Icon class="alert-icon" aria-hidden="true" />
    <div class="alert-content">
        {#if title}<p class="alert-title">{title}</p>{/if}
        <div class="alert-body">{@render children()}</div>
    </div>
</div>

<style>
    .alert {
        display: flex;
        gap: 0.625rem;
        padding: 0.75rem 0.875rem;
        border: 1px solid;
        border-radius: var(--radius-control);
        font-size: 0.875rem;
    }

    :global(.alert-icon) {
        flex-shrink: 0;
        width: 1.125rem;
        height: 1.125rem;
        margin-top: 0.125rem;
        color: var(--alert-icon-color);
    }

    .alert-content {
        min-width: 0;
    }

    .alert-title {
        margin: 0;
        font-weight: 600;
    }

    .alert-body {
        color: inherit;
    }

    .alert-body :global(p) {
        margin: 0;
        color: inherit;
    }

    .info {
        border-color: var(--border);
        background-color: var(--surface-muted);
        color: var(--text-muted);
        --alert-icon-color: var(--accent);
    }

    .success {
        border-color: color-mix(in oklab, var(--success) 35%, transparent);
        background-color: var(--success-soft);
        color: var(--success-text);
        --alert-icon-color: var(--success);
    }

    .warning {
        border-color: color-mix(in oklab, var(--warning) 35%, transparent);
        background-color: var(--warning-soft);
        color: var(--warning-text);
        --alert-icon-color: var(--warning);
    }

    .danger {
        border-color: color-mix(in oklab, var(--danger) 35%, transparent);
        background-color: var(--danger-soft);
        color: var(--danger-text);
        --alert-icon-color: var(--danger);
    }
</style>
