<script lang="ts">
import PasswordGenerator from "$lib/vault/secrets/generator/PasswordGenerator.svelte";
</script>

<svelte:head><title>Generator | Password Manager</title></svelte:head>

<div class="flex min-h-0 grow items-center justify-center overflow-auto p-6">
	<div class="w-full max-w-2xl">
		<PasswordGenerator />
	</div>
</div>
