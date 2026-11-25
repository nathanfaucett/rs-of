<script lang="ts">
import { onDestroy } from "svelte";
import { clipboardFeedback, copyValue } from "../clipboard";

let { label, value, protected: protectedValue = true }: {
    label: string;
    value: string | null | undefined;
    protected?: boolean;
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
    <legend class="mb-0 text-sm font-medium">{label}</legend>
    <p class="mb-0 whitespace-pre-wrap break-all">{value == null ? "Not set" : protectedValue && !revealed ? "••••••••" : value === "" ? "Empty value" : value}</p>
    <div class="flex gap-2">
        {#if protectedValue && value != null}
            <button class="btn btn-secondary btn-md" type="button" aria-pressed={revealed}
                onclick={() => revealed = !revealed}>{revealed ? "Hide" : "Show"} {label}</button>
        {/if}
        <button class="btn btn-secondary btn-md" type="button" disabled={copying || value == null}
            onclick={() => void copy()}>Copy {label}</button>
    </div>
    {#if copied !== null}<p role={copied ? "status" : "alert"}>{clipboardFeedback(copied)}</p>{/if}
</fieldset>
