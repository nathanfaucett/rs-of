<script lang="ts">
import LockSettingsControl from "$lib/vault/LockSettings.svelte";
import JsonExport from "$lib/vault/export/JsonExport.svelte";
import ImportJson from "$lib/vault/import/ImportJson.svelte";
import { Alert, Card } from "$lib/ui";
import { notifications } from "$lib/shared/state/notifications.svelte";
import { vaultStateModule } from "$lib/shared/state/vault.svelte";
import { Lock } from "@lucide/svelte";

let error = $state("");
let notice = $state("");
let busy = $state(false);
let importing = $state(false);
let session = 0;

let vaultData = $derived(vaultStateModule.vaultData);

function settingsSaved(settings: any) {
	if (!vaultData || vaultData.inactivity.check()) return;
	vaultData.lockSettings = settings;
}

async function downloadBackup(recovery = false) {
	if (!vaultData || busy || importing) return;
	busy = true;
	error = "";
	notice = "";
	const vault = vaultData.vault;
	const request = session;
	try {
		const backup = await (recovery ? vault.exportTransitionBackup() : vault.exportBackup());
		if (vaultData.inactivity.check() || request !== session) return;
		const url = URL.createObjectURL(new Blob([backup], { type: "application/json" }));
		const link = document.createElement("a");
		link.href = url;
		link.download = `vault-${vault.id}-${recovery ? "pre-transition" : "encrypted"}-backup.json`;
		document.body.append(link);
		try { link.click(); } finally {
			link.remove();
			setTimeout(() => URL.revokeObjectURL(url), 1000);
		}
		notice = recovery ? "Pre-transition download requested..." : "Download requested. Check that the file was saved.";
	} catch (cause) {
		if (request === session) error = cause instanceof Error ? cause.message : "Backup download failed.";
	} finally {
		if (request === session) busy = false;
	}
}

function finishImport(success = false) {
	if (!vaultData || vaultData.inactivity.check()) return;
	importing = false;
	notice = success ? "Import verified. All accepted items and folders were added." : "";
	const request = session;
	setTimeout(() => {
		if (request === session && !vaultData?.inactivity.check()) document.getElementById("import-json-action")?.focus();
	}, 0);
}

function showImport() {
	if (!vaultData || vaultData.inactivity.check() || busy) return;
	importing = true;
	notice = "";
}
</script>

<div class="flex min-h-0 grow items-start justify-center overflow-auto p-6">
	<div class="space-y-6 max-w-3xl">
		{#if error}<Alert tone="danger">{error}</Alert>{/if}
		{#if notice}<p role="status">{notice}</p>{/if}
		{#if vaultData}
		            <LockSettingsControl vault={vaultData.vault} settings={vaultData.lockSettings} onsaved={settingsSaved} />
		            <JsonExport vault={vaultData.vault} isActive={() => !vaultData.inactivity.check()} />
		            <ImportJson vault={vaultData.vault} active={() => !vaultData.inactivity.check()} oncancel={() => finishImport()} onimported={() => finishImport(true)} />
		            <div class="flex gap-2">
		                <button class="btn btn-secondary btn-md" type="button" disabled={busy || importing} onclick={() => void downloadBackup()}>{busy ? "Preparing backup…" : "Download encrypted backup"}</button>
		                <button class="btn btn-secondary btn-md" type="button" disabled={busy || importing} onclick={() => void downloadBackup(true)}>Download pre-transition backup</button>
		            </div>
		            <div class="pt-4 border-t border-line">
		                <button
		                    class="btn btn-destructive btn-md flex items-center gap-2"
		                    type="button"
		                    onclick={() => void vaultStateModule.lockVault()}
		                >
		                    <Lock class="w-4 h-4" />
		                    Lock vault
		                </button>
		            </div>
		        {/if}
	</div>
</div>
