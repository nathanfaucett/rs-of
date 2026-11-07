<script lang="ts">
import { onMount } from "svelte";
import { dev } from "$app/environment";
import { offlineError, prepareOffline } from "./offline";
import { notifications } from "$lib/common/state/notifications.svelte";

let status = $state<string | null>(null);
let busy = $state(false);
let online = $state(true);

async function check() {
	busy = true;
	try {
		if (!dev) {
			status = await prepareOffline();
		}
	} catch (cause) {
		if (!dev) status = offlineError(cause);
	} finally {
		busy = false;
	}
}

onMount(() => { void check(); });

$effect(() => {
	if (dev) return;
	if (status) {
		notifications.add(status, "info");
	}
});

$effect(() => {
	if (dev) return;
	if (!online) {
		notifications.add("Network is offline. Local vault operations do not need a server.", "info");
	} else if (online) {
		notifications.add("Back online", "success");
	}
});
</script>

<svelte:window bind:online />
