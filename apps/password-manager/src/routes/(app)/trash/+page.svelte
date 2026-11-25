<script lang="ts">
	import { vaultStateModule } from "$lib/shared/state/vault.svelte";
	import TrashItems from "./TrashItems.svelte";

	let vaultData = $derived(vaultStateModule.vaultData);
</script>

<svelte:head>
	<title>Trash | Password Manager</title>
</svelte:head>

<div class="flex min-h-0 grow items-start justify-center p-4 sm:p-6">
	{#if vaultData}
		{#key vaultData.vault}
			<TrashItems vault={vaultData.vault} />
		{/key}
	{/if}
</div>
