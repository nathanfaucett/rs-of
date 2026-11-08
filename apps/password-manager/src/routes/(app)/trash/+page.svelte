<script lang="ts">
import LocalSecretEditor from "$lib/vault/secrets/components/LocalSecretEditor.svelte";
import { vaultStateModule } from "$lib/shared/state/vault.svelte";

let vaultData = $derived(vaultStateModule.vaultData);
</script>

<div class="flex min-h-0 grow items-start justify-center overflow-auto p-6">
	{#if vaultData}
		<LocalSecretEditor vault={vaultData.vault} initialTrash={true} />
	{/if}
</div>
