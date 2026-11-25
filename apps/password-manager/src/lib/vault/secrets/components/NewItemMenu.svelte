<script lang="ts">
import { tick } from "svelte";
import type { EditableKind } from "../secret-draft";

let { disabled, onselect }: { disabled: boolean; onselect: (kind: EditableKind) => void } = $props();

const kinds = [
    ["login", "New login"],
    ["secureNote", "New secure note"],
    ["card", "New card"],
    ["identity", "New identity"],
    ["sshKey", "New SSH key"],
] as const;
const id = $props.id();
let open = $state(false);
let trigger: HTMLButtonElement;
let menu = $state<HTMLDivElement>();

function focusItem(index: number) {
    menu?.querySelectorAll<HTMLButtonElement>("button")[index]?.focus();
}

async function openMenu(index: number) {
    open = true;
    await tick();
    focusItem(index);
}

function closeMenu(restoreFocus = true) {
    open = false;
    if (restoreFocus) trigger.focus();
}

function handleOutsideClick(event: MouseEvent) {
    if (open && event.target instanceof Node && !trigger.contains(event.target) && !menu?.contains(event.target)) {
        const focused = document.activeElement;
        closeMenu(focused === document.body || menu?.contains(focused));
    }
}

function handleItemKeydown(event: KeyboardEvent, index: number) {
    switch (event.key) {
        case "Tab": closeMenu(); return;
        case "Escape": closeMenu(); break;
        case "ArrowDown": focusItem((index + 1) % kinds.length); break;
        case "ArrowUp": focusItem((index + kinds.length - 1) % kinds.length); break;
        case "Home": focusItem(0); break;
        case "End": focusItem(kinds.length - 1); break;
        default: return;
    }
    event.preventDefault();
}
</script>

<svelte:document onclick={handleOutsideClick} />

<div class="relative">
    <button id={`${id}-trigger`} bind:this={trigger} class="btn btn-primary btn-md" type="button" {disabled}
        aria-haspopup="menu" aria-expanded={open} aria-controls={open ? id : undefined}
        onclick={() => { if (open) closeMenu(); else void openMenu(0); }}
        onkeydown={(event) => {
            if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
            event.preventDefault();
            void openMenu(event.key === "ArrowUp" ? kinds.length - 1 : 0);
        }}>New item</button>
    {#if open}
        <div {id} bind:this={menu} class="absolute top-full right-0 z-10 mt-1 w-max min-w-40 rounded-lg border border-line bg-surface py-1 shadow-lg" role="menu" aria-labelledby={`${id}-trigger`}>
            {#each kinds as [kind, label], index (kind)}
                <button class="block w-full px-3 py-2 text-left text-sm text-ink hover:bg-surface-hover focus:bg-surface-hover"
                    role="menuitem" type="button" tabindex="-1" onkeydown={(event) => handleItemKeydown(event, index)}
                    onclick={() => { closeMenu(); onselect(kind); }}>{label}</button>
            {/each}
        </div>
    {/if}
</div>
