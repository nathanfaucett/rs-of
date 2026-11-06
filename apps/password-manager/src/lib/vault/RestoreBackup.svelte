<script lang="ts">
import { onDestroy } from "svelte";
import { Alert, Card, Field } from "$lib/ui";
import { BACKUP_MAX_BYTES, type BackupRestore, prepareBackupRestore } from "./backup";
import { listVaults, type VaultIdentity } from "./vault";

let { onrestored, oncancel }: { onrestored: (identity: VaultIdentity) => Promise<void>; oncancel: () => void } = $props();
let files = $state<FileList>();
let password = $state("");
let prepared = $state.raw<BackupRestore>();
let collision = $state(false);
let busy = $state(false);
let error = $state("");
let active = true;
onDestroy(() => { active = false; prepared = undefined; password = ""; });

async function validate() {
    if (busy) return;
    error = "";
    const file = files?.[0];
    if (!file || !password) { error = "Select a backup file and enter its master password."; return; }
    if (file.size > BACKUP_MAX_BYTES) { error = "Backup exceeds the 16 MiB limit. Keep the original file."; return; }
    busy = true;
    try {
        const result = await prepareBackupRestore(password, await file.text());
        const exists = (await listVaults()).some((entry) => entry.id === result.id);
        if (!active) return;
        password = "";
        collision = exists;
        prepared = result;
    } catch (cause) {
        if (active) error = cause instanceof Error ? cause.message : "Cannot read the backup. Keep the original file and retry.";
    } finally {
        if (active) busy = false;
    }
}
async function restore() {
    if (busy || !prepared || collision) return;
    error = "";
    busy = true;
    try {
        const identity = await prepared.restore();
        if (active) await onrestored(identity);
    } catch (cause) {
        if (active) error = cause instanceof Error ? cause.message : "Restore failed. Keep the backup and check the selector before retrying.";
    } finally {
        if (active) busy = false;
    }
}
</script>

<Card>
    <div class="flex flex-col gap-4">
        <h2 class="text-lg font-semibold">Restore encrypted backup</h2>
        <p>Restore creates a complete local vault and never overwrites an existing vault ID. Use the master password used when the backup was made. Keep the file until you verify the restored data.</p>
        {#if error}<Alert tone="danger">{error}</Alert>{/if}
        {#if prepared}
            <p role="status">Validated backup: {prepared.name}. ID: {prepared.id}. {prepared.recordCount} encrypted records.</p>
            {#if collision}
                <Alert tone="danger">This vault ID already exists. Select the existing vault, or restore in a clean browser profile. Do not delete the source vault.</Alert>
            {/if}
            <button class="btn btn-primary btn-md" type="button" disabled={busy || collision} onclick={restore}>{busy ? "Restoring…" : "Restore vault"}</button>
        {:else}
            <Field id="backup-file" label="Encrypted backup file" required>
                {#snippet children(control)}
                    <input {...control} type="file" accept=".json,application/json" bind:files disabled={busy} />
                {/snippet}
            </Field>
            <Field id="backup-password" label="Backup master password" required>
                {#snippet children(control)}
                    <input {...control} type="password" autocomplete="off" bind:value={password} disabled={busy} />
                {/snippet}
            </Field>
            <button class="btn btn-primary btn-md" type="button" disabled={busy} onclick={validate}>{busy ? "Validating…" : "Validate backup"}</button>
        {/if}
        <button class="btn btn-secondary btn-md" type="button" disabled={busy} onclick={oncancel}>Cancel restore</button>
    </div>
</Card>
