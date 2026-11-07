<script lang="ts">
import { onDestroy } from "svelte";
import { Alert, Card, Field } from "$lib/ui";
import type { VaultHandle } from "../vault";
import { preparePersonalJsonImport, type ImportPreview } from "./import";
import { IMPORT_MAX_BYTES } from "./personal-json";

let { vault, active, oncancel, onimported }: {
    vault: VaultHandle;
    active: () => boolean;
    oncancel: () => void;
    onimported: () => void;
} = $props();
let file: File | null = $state.raw(null);
let preview = $state.raw<ImportPreview | null>(null);
let busy = $state(false);
let error = $state("");
let confirmed = $state(false);
let retired = $state(false);
let generation = 0;

const duplicate = $derived(!!preview && !!(preview.duplicates.sourceIds || preview.duplicates.logical));
export function retire() {
    retired = true;
    generation++;
    preview?.retire();
    preview = null;
    file = null;
    confirmed = false;
    error = "";
}
onDestroy(retire);
function current(request: number) { return !retired && request === generation && active(); }
function select(event: Event) {
    generation++;
    preview?.retire(); preview = null; confirmed = false; error = "";
    const input = event.currentTarget as HTMLInputElement;
    file = input.files?.[0] ?? null;
    if (file && file.size > IMPORT_MAX_BYTES) {
        file = null; input.value = "";
        error = "File exceeds 16 MiB. Split it into smaller supported JSON files.";
    }
}
async function validate() {
    if (!file || busy || !active()) return;
    const request = ++generation;
    const selected = file;
    busy = true; error = "";
    try {
        const text = new TextDecoder("utf-8", { fatal: true }).decode(await selected.arrayBuffer());
        if (!current(request)) return;
        const prepared = await preparePersonalJsonImport(vault, text);
        if (!current(request)) { prepared.retire(); return; }
        preview = prepared;
        file = null;
        confirmed = false;
    } catch {
        if (current(request)) error = "Cannot validate this UTF-8 personal JSON file. Check the file and local storage; do not clear browser data.";
    } finally { if (current(request)) busy = false; }
}
async function commit() {
    if (!preview || busy || !active() || (duplicate && !confirmed)) return;
    const request = generation;
    const prepared = preview;
    busy = true; error = "";
    try {
        await prepared.commit(confirmed ? "import-as-new" : undefined);
        if (current(request)) onimported();
    } catch {
        if (current(request)) {
            preview?.retire(); preview = null;
            error = "Import could not be confirmed. No partial batch is published. Lock, reopen, and verify before retrying: the old or complete new batch can be present. Do not clear browser storage.";
        }
    } finally { if (current(request)) busy = false; }
}
</script>

<svelte:window onbeforeunload={(event) => {
    if (busy || preview || file) { event.preventDefault(); event.returnValue = ""; }
}} />

{#if !retired}
<Card>
    <h2 tabindex="-1" {@attach (element) => { element.focus(); }}>Import Bitwarden JSON</h2>
    <p>Destination vault: <span class="break-all">{vault.id}</span></p>
    <p>Plaintext files expose credentials. Use a trusted personal JSON file and protect or remove it after migration. Nothing is sent to a server. This is not an encrypted backup.</p>
    <p>Supported subset: Login, Secure Note, Card, Identity, SSH Key, folders, custom fields and history. Attachments, organizations, collections, Trash, archive, linked fields, CSV, encrypted exports and unknown fields block the whole import. Passkeys are not supported: they are skipped and listed as warnings. Limit: 16 MiB, 10,000 items and 10,000 folders.</p>
    {#if error}<Alert tone="danger">{error}</Alert>{/if}
    {#if preview}
        <h3 tabindex="-1" {@attach (element) => { element.focus(); }}>Import preview</h3>
        <p>No items or folders have been written. Existing items, Trash, folders and lock settings are retained. Every accepted record gets a new local identity; names never authorize overwrite.</p>
        <ul aria-label="Import counts">
            <li>Login: {preview.counts.login}</li>
            <li>Secure Note: {preview.counts.secureNote}</li>
            <li>Card: {preview.counts.card}</li>
            <li>Identity: {preview.counts.identity}</li>
            <li>SSH Key: {preview.counts.sshKey}</li>
            <li>Folders: {preview.counts.folders}</li>
        </ul>
        <p>Source-ID matches: {preview.duplicates.sourceIds}. Logical duplicates: {preview.duplicates.logical}. Counts can overlap. Cancel is the default; no skip, merge or overwrite.</p>
        {#if preview.warnings.length}
            <ul aria-label="Import warnings">{#each preview.warnings as warning (warning)}<li>{warning}</li>{/each}</ul>
        {/if}
        {#if preview.issues.length}
            <Alert tone="danger" title="Import blocked">Correct every issue in a copy of the source file. No supported-only partial import is available.</Alert>
            <ul aria-label="Import issues">{#each preview.issues as issue, index (`${index}:${issue.path}`)}
                <li><code>{issue.path}</code>: {issue.unsupported ? "Unsupported — " : "Validation — "}{issue.message}</li>
            {/each}</ul>
        {:else}
            {#if duplicate}
                <label><input type="checkbox" aria-label="Import duplicates as new" bind:checked={confirmed} disabled={busy} /> Import all duplicates as new records and folders. Keep the originals.</label>
            {/if}
            <button class="btn btn-primary btn-md" type="button" disabled={busy || (duplicate && !confirmed)} onclick={commit}>Import all as new</button>
        {/if}
    {:else}
        <Field id="import-file" label="Personal JSON file">
            {#snippet children(control)}<input {...control} type="file" accept=".json,application/json" disabled={busy} onchange={select} />{/snippet}
        </Field>
        <button class="btn btn-primary btn-md" type="button" disabled={busy || !file} onclick={validate}>Validate import</button>
    {/if}
    {#if busy}<p role="status">{preview ? "Importing complete batch… A sent write can finish after lock." : "Validating import…"}</p>{/if}
    <button class="btn btn-secondary btn-md" type="button" disabled={busy} onclick={() => { retire(); oncancel(); }}>Cancel import</button>
</Card>
{/if}
