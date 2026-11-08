<script lang="ts">
	import type { Snippet } from "svelte";

	interface Props {
		title: string;
		children: Snippet;
		open?: boolean;
		onclose?: () => void;
	}

	let { title, children, open = false, onclose }: Props = $props();
	let dialog = $state<HTMLDialogElement>();

	const handleClose = () => {
		dialog?.close();
		onclose?.();
	};

	$effect(() => {
		if (open && dialog) {
			dialog.showModal();
		} else if (!open && dialog) {
			dialog.close();
		}
	});

	export { handleClose };
</script>

<dialog
	bind:this={dialog}
	class="m-auto w-full max-w-lg rounded-control border border-line bg-surface p-5 text-ink shadow-lg backdrop:bg-surface-inverse/60"
	aria-labelledby="dialog-title"
	onclose={handleClose}
>
	<div class="flex flex-col gap-4">
		<div class="flex items-center justify-between gap-4">
			<h2 id="dialog-title" class="mb-0 text-xl font-semibold">{title}</h2>
			<form method="dialog">
				<button
					type="submit"
					class="btn btn-ghost btn-md"
					aria-label="Close"
				>
					<svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
						<path d="M18 6 6 18" />
						<path d="m6 6 12 12" />
					</svg>
				</button>
			</form>
		</div>
		{@render children()}
	</div>
</dialog>
