<script lang="ts">
	import { onMount } from "svelte";
	import PasswordGenerator from "./PasswordGenerator.svelte";
	import Dialog from "$lib/shared/components/Dialog.svelte";

	let { onuse }: { onuse?: (value: string) => void } = $props();

	let dialogOpen = $state(false);
	let generator: PasswordGenerator;

	function open() {
		dialogOpen = true;
	}

	function close() {
		dialogOpen = false;
	}

	function handleUse(value: string) {
		onuse?.(value);
		close();
	}

	$effect(() => {
		if (dialogOpen && generator) {
			generator.generate();
		}
	});
</script>

<Dialog title="Password Generator" open={dialogOpen} onclose={close}>
	<PasswordGenerator bind:this={generator} onuse={handleUse} />
</Dialog>

<div>
	<button class="btn btn-secondary btn-md" type="button" onclick={open}>
		<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
			<path d="m15.5 7.5 2.3 2.3a1 1 0 0 0 1.4 0l2.1-2.1a1 1 0 0 0 0-1.4L19 4" />
			<path d="m21 2-9.6 9.6" />
			<circle cx="7.5" cy="15.5" r="5.5" />
		</svg>
		<span class="ml-2">Generate</span>
	</button>
</div>
