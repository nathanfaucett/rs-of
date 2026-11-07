<script lang="ts">
import { onDestroy, tick } from "svelte";
import type { VaultHandle } from "../vault";
import { inspectJsonExport, JsonExportError, serializeJsonExport } from "./export";

let { vault, isActive }: { vault: VaultHandle; isActive: () => boolean } = $props();
let preview = $state.raw<ReturnType<typeof inspectJsonExport> | null>(null);
let busy = $state(false);
let opened = $state(false);
let error = $state("");
let notice = $state("");
let trigger: HTMLButtonElement | undefined;
let cancelButton: HTMLButtonElement | undefined;
let request = 0;
let retired = $state(false);
let url: string | undefined;
let revokeTimer: ReturnType<typeof setTimeout> | undefined;
function revoke() {
    clearTimeout(revokeTimer);
    if (url) URL.revokeObjectURL(url);
    url = undefined;
}
export function retire() {
    retired = true;
    request++;
    preview = null;
    opened = false;
    busy = false;
    error = "";
    notice = "";
    revoke();
}
onDestroy(retire);
function current(token: number) {
    return isActive() && !retired && token === request;
}
async function open() {
    if (retired || busy || !isActive()) return;
    const token = ++request;
    opened = true;
    busy = true;
    error = "";
    notice = "";
    preview = null;
    await tick();
    if (!current(token)) return;
    cancelButton?.focus();
    try {
        const snapshot = await vault.readItems();
        if (!current(token)) return;
        preview = inspectJsonExport(snapshot);
    } catch (cause) {
        if (current(token)) error = cause instanceof JsonExportError ? cause.message : "Export blocked. Unsupported or unreadable vault data. Keep the source vault and check an encrypted backup.";
    } finally {
        if (current(token)) busy = false;
    }
}
async function cancel() {
    const token = ++request;
    preview = null;
    opened = false;
    busy = false;
    error = "";
    await tick();
    if (current(token) && !opened) trigger?.focus();
}
function download() {
    const token = request;
    if (!current(token) || !preview || busy) return;
    let link: HTMLAnchorElement | undefined;
    try {
        const content = serializeJsonExport(preview.snapshot);
        if (!current(token)) return;
        revoke();
        url = URL.createObjectURL(new Blob([content], { type: "application/json;charset=utf-8" }));
        if (!current(token)) { revoke(); return; }
        link = document.createElement("a");
        link.href = url;
        link.download = `vault-${vault.id}-plaintext.json`;
        document.body.append(link);
        if (!current(token)) { revoke(); return; }
        link.click();
        revokeTimer = setTimeout(revoke, 1000);
        void cancel();
        notice = "Plaintext download requested. Check the saved file and keep it secure. The browser cannot confirm that it was saved.";
    } catch {
        revoke();
        if (current(token)) error = "Plaintext export failed. No saved file is confirmed. Keep the source vault and retry.";
    } finally {
        link?.remove();
    }
}
</script>

{#if !retired}
    <button {@attach (element) => { trigger = element; return () => { trigger = undefined; }; }} class="btn btn-secondary btn-md" type="button" disabled={opened} onclick={open}>Export Bitwarden JSON</button>
    {#if opened}
        <section aria-labelledby="json-export-title" class="my-4 flex flex-col gap-3 rounded border p-4">
            <h2 id="json-export-title">Plaintext credential export</h2>
            <p>This file is not encrypted. Anyone with the file can read passwords, private keys, notes, history, and other credentials. It is not a recovery backup. Use Download encrypted backup for recovery.</p>
            <p>Scope: all active personal items in the committed snapshot at preview, not the current search or folder filter. All folders, including empty folders, are included. Unsaved edits are not included.</p>
            <p>Exclusions: Trash, attachments, passkeys, Sends, account defaults, import provenance, and vault lock settings. Organization permissions are not recreated. Unsupported active data blocks export; no fields are silently dropped.</p>
            {#if preview}
                <p role="status">{preview.active} active items; {preview.folders} folders; {preview.trash} Trash items excluded.</p>
                <p>Login: {preview.counts.login}; Secure Note: {preview.counts.secureNote}; Card: {preview.counts.card}; Identity: {preview.counts.identity}; SSH Key: {preview.counts.sshKey}.</p>
                <p>Supported logical values only, not a full account backup or verified acceptance by the selected reference client.</p>
            {:else if busy}
                <p role="status">Checking export scope…</p>
            {/if}
            {#if error}<p role="alert">{error}</p>{/if}
            <div class="flex gap-2">
                <button {@attach (element) => { cancelButton = element; return () => { cancelButton = undefined; }; }} class="btn btn-secondary btn-md" type="button" onclick={cancel}>Cancel export</button>
                <button class="btn btn-primary btn-md" type="button" disabled={busy || !preview} onclick={download}>Download plaintext JSON</button>
            </div>
        </section>
    {/if}
    {#if notice}<p role="status">{notice}</p>{/if}
{/if}
