<script lang="ts">
import { onDestroy } from "svelte";
import { parse } from "valibot";
import type { VaultHandleLike } from "$lib/vault/types";
import { LockSettingsSchema, type LockSettings, LOCK_TIMEOUT_OPTIONS } from "./lock";

let { vault, settings, onsaved }: {
    vault: VaultHandleLike;
    settings: LockSettings;
    onsaved: (settings: LockSettings) => void;
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
        const confirmed = await vault.saveLockSettings(parse(LockSettingsSchema, { timeoutMinutes: timeout }));
        if (!active) return;
        onsaved(confirmed);
        if (!active) return;
        failed = false;
        feedback = "Lock settings saved.";
    } catch {
        if (!active) return;
        failed = true;
        feedback = "Lock settings could not be confirmed. Lock, reopen, and verify before retrying. The previous session timeout remains active.";
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
    <p>Saved in this encrypted vault. Each unlock starts a new deadline. Reload requires your master password.</p>
    <button class="btn btn-secondary btn-md" type="button" disabled={busy} onclick={() => void save()}>{busy ? "Saving lock settings…" : "Save lock settings"}</button>
    {#if feedback}<p role={failed ? "alert" : "status"}>{feedback}</p>{/if}
</fieldset>
