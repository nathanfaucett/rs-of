<script lang="ts">
    import type { Snippet } from "svelte";

    interface Props {
        title: string;
        onclose?: () => void;
        children: Snippet;
    }

    let { title, children, onclose }: Props = $props();
    const titleId = $props.id();
    let dialog = $state<HTMLDialogElement>();

    export function show() {
        dialog?.showModal();
    }

    export function close() {
        dialog?.close();
    }
</script>

<dialog
    bind:this={dialog}
    {onclose}
    class="m-auto w-full max-w-lg rounded-card border border-line bg-surface p-5 text-ink shadow-lg backdrop:bg-surface-inverse/60"
    aria-labelledby={titleId}
>
    <div class="flex flex-col gap-4">
        <div class="flex items-center justify-between gap-4">
            <h2 id={titleId} class="mb-0 text-2xl">{title}</h2>
            <form method="dialog">
                <button
                    type="submit"
                    class="btn btn-secondary btn-md"
                    aria-label="Close">Close</button
                >
            </form>
        </div>
        {@render children()}
    </div>
</dialog>
