<script lang="ts">
import type { Snippet } from "svelte";

interface Props {
	icon: Snippet;
	label: string;
	active?: boolean;
	collapsed?: boolean;
	onclick?: () => void;
	onkeydown?: (e: KeyboardEvent) => void;
}

let { icon, label, active = false, collapsed = false, onclick, onkeydown }: Props = $props();
</script>

{#snippet tooltip()}
	{#if collapsed}
		<span class="absolute left-full top-1/2 -translate-y-1/2 ml-2 px-2 py-1 text-xs bg-muted/95 backdrop-blur text-muted-foreground rounded border border-line whitespace-nowrap shadow-lg z-10">
			{label}
		</span>
	{/if}
{/snippet}

<button
	role="menuitem"
	aria-current={active ? "page" : undefined}
	aria-label={label}
	class="relative flex w-full items-center gap-3 px-3 py-2 text-sm font-medium transition-colors rounded-lg
		{active ? 'bg-accent-soft text-accent-text' : 'text-muted-foreground hover:bg-surface-hover hover:text-ink'}
		{collapsed ? 'justify-center' : 'justify-start'}"
	onclick={onclick}
	onkeydown={onkeydown}
>
	{@render icon()}
	{@render tooltip()}
	{#if !collapsed}
		<span class="truncate">{label}</span>
	{/if}
</button>
