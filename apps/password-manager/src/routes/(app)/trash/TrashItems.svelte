<script lang="ts">
	import { onDestroy, tick, untrack } from "svelte";
	import Dialog from "$lib/shared/components/Dialog.svelte";
	import { formatTimestamp } from "$lib/shared/util/timestamp";
	import { Alert, Card } from "$lib/ui";
	import type { Secret } from "$lib/vault/secrets/secret";
	import { createLocalSecretWorkflow } from "$lib/vault/secrets/workflow.svelte";
	import type { VaultHandleLike } from "$lib/vault/types";

	let { vault }: { vault: VaultHandleLike } = $props();
	const workflow = untrack(() => createLocalSecretWorkflow({ vault }));
	onDestroy(workflow.dispose);

	let heading: HTMLHeadingElement;
	let cancelButton = $state<HTMLButtonElement>();
	let pendingDelete = $state.raw<{ item: Secret; trigger: HTMLButtonElement }>();

	async function requestDelete(item: Secret, trigger: HTMLButtonElement) {
		pendingDelete = { item, trigger };
		await tick();
		cancelButton?.focus();
	}

	async function cancelDelete() {
		const trigger = pendingDelete?.trigger;
		pendingDelete = undefined;
		await tick();
		trigger?.focus();
	}

	async function changeItem(item: Secret, trigger: HTMLButtonElement, purge: boolean) {
		pendingDelete = undefined;
		if (purge) await workflow.deletePermanently(item);
		else await workflow.restore(item);
		await tick();
		(trigger.isConnected && !trigger.disabled ? trigger : heading)?.focus();
	}
</script>

<div class="w-full min-w-0 max-w-3xl">
	<Card>
		<header class="mb-4 flex flex-wrap items-start justify-between gap-3">
			<div>
				<h1 bind:this={heading} tabindex="-1" class="text-xl font-semibold">Trash</h1>
				<p class="text-sm text-ink-subtle">Items stay in Trash until you restore or permanently delete them.</p>
			</div>
			<button class="btn btn-secondary btn-md" type="button" disabled={!workflow.canRefresh}
				onclick={() => void workflow.refresh()}>Refresh</button>
		</header>

		{#if workflow.refreshing}
			<p role="status">Refreshing Trash…</p>
		{/if}
		{#if workflow.loadError}
			<Alert tone="danger" title="Vault unavailable">{workflow.loadError}</Alert>
		{:else if workflow.loading}
			<p role="status">Loading Trash…</p>
		{:else}
			{#if workflow.error}
				<Alert tone="danger" title="Action failed">{workflow.error}</Alert>
			{/if}
			{#if workflow.busy && !workflow.refreshing}
				<p role="status">Saving changes…</p>
			{:else if workflow.notice}
				<Alert tone="success">{workflow.notice}</Alert>
			{/if}

			{#if pendingDelete}
				<Dialog title="Delete permanently?" open onclose={cancelDelete}>
					<p class="wrap-anywhere">Delete {pendingDelete.item.name} permanently? You cannot undo this action. Existing backups can still contain this item.</p>
					<div class="flex flex-wrap justify-end gap-2">
						<button bind:this={cancelButton} class="btn btn-secondary btn-md" type="button" onclick={cancelDelete}>Cancel</button>
						<button class="btn btn-danger btn-md" type="button" disabled={!workflow.canEdit}
							onclick={() => { if (pendingDelete) void changeItem(pendingDelete.item, pendingDelete.trigger, true); }}>
							Delete permanently
						</button>
					</div>
				</Dialog>
			{/if}

			{#if workflow.trashRecords.length === 0}
				<p role="status">Trash is empty.</p>
			{:else}
				<ul aria-label="Trash items" class="flex list-none flex-col gap-2 p-0">
					{#each workflow.trashRecords as item (item.id)}
						<li class="flex flex-wrap items-center justify-between gap-3 rounded-control border border-line bg-surface-muted/40 px-3 py-2">
							<div class="min-w-0 wrap-anywhere">
								<p class="font-medium">{item.name}</p>
								{#if item.deletedAt}
									{@const formatted = formatTimestamp(item.deletedAt)}
									<p class="text-sm text-ink-subtle">Deleted: {#if formatted}<time datetime={item.deletedAt}>{formatted}</time>{:else}Unknown{/if}</p>
								{/if}
							</div>
							<div class="flex flex-wrap gap-2">
								<button class="btn btn-secondary btn-md" type="button" disabled={!workflow.canEdit} aria-label={`Restore: ${item.name}`}
									onclick={(event) => void changeItem(item, event.currentTarget, false)}>Restore</button>
								<button class="btn btn-danger btn-md" type="button" disabled={!workflow.canEdit} aria-label={`Delete permanently: ${item.name}`}
									onclick={(event) => void requestDelete(item, event.currentTarget)}>Delete permanently</button>
							</div>
						</li>
					{/each}
				</ul>
			{/if}
		{/if}
	</Card>
</div>
