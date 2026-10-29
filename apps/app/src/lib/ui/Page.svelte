<script lang="ts" module>
    export type PageWidth = "wide" | "narrow";

    export interface PageProps {
        width?: PageWidth;
        children: import("svelte").Snippet;
    }
</script>

<script lang="ts">
    let { width = "wide", children }: PageProps = $props();
</script>

<div class="page {width}">
    {@render children()}
</div>

<style>
    .page {
        display: flex;
        flex-direction: column;
        gap: 1.25rem;
        width: 100%;
        max-width: 72rem;
        margin-inline: auto;
        padding-inline: 1rem;
    }

    .page.narrow {
        max-width: 34rem;
    }

    @media (min-width: 40rem) {
        .page {
            padding-inline: 1.5rem;
        }
    }
</style>
