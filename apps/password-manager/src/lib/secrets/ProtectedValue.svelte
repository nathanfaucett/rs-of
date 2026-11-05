<script lang="ts">
import { onDestroy } from "svelte";

let { label, value, protected: protectedValue = true }: {
    label: string;
    value: string | null | undefined;
    protected?: boolean;
} = $props();

let revealed = $state(false);
let copying = $state(false);
let feedback = $state("");
let failed = $state(false);
let active = true;
onDestroy(() => { active = false; });

async function copy() {
    if (copying || value == null) return;
    copying = true;
    feedback = "";
    try {
        if (!navigator.clipboard?.writeText) throw new Error();
        await navigator.clipboard.writeText(value);
        if (active) { failed = false; feedback = "Copied. Clipboard is not cleared automatically."; }
    } catch {
        if (active) { failed = true; feedback = "Copy failed. Clipboard access is unavailable or denied. Reveal the value and copy it manually."; }
    } finally {
        if (active) copying = false;
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
    {#if feedback}<p role={failed ? "alert" : "status"}>{feedback}</p>{/if}
</fieldset>
