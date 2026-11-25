<script lang="ts">
import { onDestroy } from "svelte";
import { parse } from "valibot";

import { LOCK_TIMEOUT_OPTIONS, type LockSettings, LockSettingsSchema } from "./lock";

let { settings, onsave }: {
    settings: LockSettings;
    onsave: (settings: LockSettings) => Promise<boolean>;
} = $props();
let selected = $state<number | null>(null);
let timeout = $derived(selected ?? settings.timeoutMinutes);
let busy = $state(false);
let feedback = $state("");
let failed = $state(false);
let active = true;
onDestroy(() => { active = false; });

async function save() {
    if (!active || busy) return;
    busy = true;
    feedback = "";
    try {
        const applied = await onsave(parse(LockSettingsSchema, { timeoutMinutes: timeout }));
        if (!active) return;
        selected = null;
        if (applied) {
            failed = false;
            feedback = "Lock settings saved.";
        }
    } catch {
        if (!active) return;
        selected = null;
        failed = true;
        feedback = "Could not confirm the lock settings. The previous timeout is still active. Lock and reopen the vault to check the saved value before you retry.";
    } finally {
        if (active) busy = false;
    }
}
</script>

<fieldset class="mb-4 flex flex-col gap-2">
    <legend>Automatic lock</legend>
    <label for="lock-timeout">Lock after inactivity</label>
    <select id="lock-timeout" value={timeout} onchange={(event) => selected = Number(event.currentTarget.value)} disabled={busy}>
        {#each LOCK_TIMEOUT_OPTIONS as opt (opt.value)}
            <option value={opt.value}>{opt.label}</option>
        {/each}
    </select>
    <p>Saving applies the timeout now and stores it in this encrypted vault. Reload requires your master password.</p>
    <button class="btn btn-secondary btn-md" type="button" disabled={busy} onclick={() => void save()}>{busy ? "Saving lock settings…" : "Save lock settings"}</button>
    {#if feedback}<p role={failed ? "alert" : "status"}>{feedback}</p>{/if}
</fieldset>
