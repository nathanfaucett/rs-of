<script lang="ts">
import { Copy, Eye, EyeOff } from "@lucide/svelte";
import { onDestroy } from "svelte";
import type { HTMLInputAttributes } from "svelte/elements";
import { clipboardFeedback, copyValue } from "../clipboard";

type Props = Pick<
    HTMLInputAttributes,
    "id" | "required" | "aria-describedby" | "aria-invalid"
> & {
    label: string;
    value: string | null;
    disabled?: boolean;
    autocomplete?: HTMLInputAttributes["autocomplete"];
    rows?: number | undefined;
};

let { label, value = $bindable(), disabled = false, autocomplete = "off", rows, ...control }: Props = $props();
let revealed = $state(false);
let copying = $state(false);
let copied = $state<boolean | null>(null);
let active = true;
onDestroy(() => { active = false; });

async function copy() {
    if (copying || !value) return;
    copying = true;
    copied = null;
    const result = await copyValue(value);
    if (active) {
        copied = result;
        copying = false;
    }
}
</script>

<div class="min-w-0 grow">
    <div class="relative">
        {#if rows === undefined}
            <input {...control} type={revealed ? "text" : "password"} bind:value
                {autocomplete} {disabled} spellcheck="false" autocapitalize="off" class="w-full pr-20" />
        {:else if revealed}
            <textarea {...control} {@attach (element) => element.focus()} bind:value {rows} {autocomplete} {disabled}
                spellcheck="false" autocapitalize="off" class="w-full"></textarea>
        {:else}
            <p class="mb-0">{value ? "••••••••" : "Not set"}</p>
        {/if}
        <div class={rows === undefined ? "absolute inset-y-0 right-0 flex items-center gap-1 pr-1" : "flex flex-wrap gap-2"}>
            <button type="button" class="btn btn-ghost btn-sm" onclick={copy} aria-label={`Copy ${label}`}
                disabled={disabled || copying || !value}>
                <Copy class="size-5" />
            </button>
            <button type="button" class="btn btn-ghost btn-sm" onclick={() => revealed = !revealed}
                aria-label={`${revealed ? "Hide" : "Show"} ${label}`} aria-pressed={revealed} {disabled}>
                {#if revealed}<EyeOff class="size-5" />{:else}<Eye class="size-5" />{/if}
                {#if rows !== undefined}{revealed ? "Hide" : "Show"} {label}{/if}
            </button>
        </div>
    </div>
    {#if copied !== null}<p role={copied ? "status" : "alert"}>{clipboardFeedback(copied)}</p>{/if}
</div>
