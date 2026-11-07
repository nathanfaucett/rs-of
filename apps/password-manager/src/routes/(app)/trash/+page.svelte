<script lang="ts">
import LocalSecretEditor from "$lib/secrets/LocalSecretEditor.svelte";

interface Props {
	data: {
		vault: any;
	};
}

let { data }: Props = $props();
</script>

<div class="flex min-h-0 grow items-start justify-center overflow-auto p-6">
	<LocalSecretEditor vault={data.vault} initialTrash={true} hideTrashToggle={true} />
</div>
