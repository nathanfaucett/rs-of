<script lang="ts">
import { onDestroy, type Snippet } from "svelte";
import { clipboardFeedback, copyValue } from "../clipboard";

let { label, value, protected: protectedValue = true, actions }: {
    label: string;
    value: string | null | undefined;
    protected?: boolean;
    actions?: Snippet;
} = $props();

let revealed = $state(false);
let copying = $state(false);
let copied = $state<boolean | null>(null);
let active = true;
onDestroy(() => { active = false; });

async function copy() {
    if (copying || value == null) return;
    copying = true;
    copied = null;
    const result = await copyValue(value);
    if (active) {
        copied = result;
        copying = false;
    }
}
</script>

<fieldset class="flex min-w-0 flex-col gap-2">
    <legend class="mb-0 max-w-full wrap-anywhere text-sm font-medium">{label}</legend>
    <p class="mb-0 whitespace-pre-wrap break-all">{value == null ? "Not set" : protectedValue && !revealed ? "••••••••" : value === "" ? "Empty value" : value}</p>
    <div class="flex flex-wrap gap-2">
        {#if protectedValue && value != null}
            <button class="btn btn-secondary btn-md h-auto min-h-9 max-w-full py-1.5 text-center wrap-anywhere whitespace-normal" type="button" aria-pressed={revealed}
                onclick={() => revealed = !revealed}>{revealed ? "Hide" : "Show"} {label}</button>
        {/if}
        <button class="btn btn-secondary btn-md h-auto min-h-9 max-w-full py-1.5 text-center wrap-anywhere whitespace-normal" type="button" disabled={copying || value == null}
            onclick={() => void copy()}>Copy {label}</button>
        {@render actions?.()}
    </div>
    {#if copied !== null}<p role={copied ? "status" : "alert"}>{clipboardFeedback(copied)}</p>{/if}
</fieldset>
