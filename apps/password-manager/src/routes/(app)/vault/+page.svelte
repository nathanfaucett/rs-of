<script lang="ts">
import { onMount } from "svelte";
import { page } from "$app/state";
import LocalSecretEditor from "$lib/secrets/LocalSecretEditor.svelte";
import { Alert } from "$lib/ui";
import { notifications } from "$lib/common/state/notifications.svelte";

interface Props {
	data: {
		vault: any;
		inactivity: any;
		lockSettings: any;
	};
}

let { data }: Props = $props();

let error = $state("");
let notice = $state("");
let itemEditor = $state<ReturnType<typeof LocalSecretEditor>>();

onMount(() => {
	// Initialize vault-specific state
});
</script>

<div class="flex min-h-0 grow items-start justify-center overflow-auto p-6">
	{#if error}
		<Alert tone="danger">{error}</Alert>
	{/if}
	{#if notice}
		<p role="status">{notice}</p>
	{/if}
	<LocalSecretEditor bind:this={itemEditor} vault={data.vault} />
</div>
