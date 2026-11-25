<script lang="ts">
import { vaultStateModule } from "$lib/shared/state/vault.svelte";
import LocalSecretEditor from "$lib/vault/secrets/components/LocalSecretEditor.svelte";

let vaultData = $derived(vaultStateModule.vaultData);
</script>

<svelte:head><title>Trash | Password Manager</title></svelte:head>

<div class="flex min-h-0 grow items-start justify-center overflow-auto p-6">
	{#if vaultData}
		<LocalSecretEditor vault={vaultData.vault} initialTrash={true} />
	{/if}
</div>
