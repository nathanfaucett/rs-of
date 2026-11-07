<script lang="ts">
import LockSettingsControl from "$lib/vault/LockSettings.svelte";
import JsonExport from "$lib/json-export/JsonExport.svelte";
import ImportJson from "$lib/import/ImportJson.svelte";
import { Alert, Card } from "$lib/ui";
import { notifications } from "$lib/common/state/notifications.svelte";

interface Props {
	data: {
		vault: any;
		inactivity: any;
		lockSettings: any;
	};
}

let { data }: Props = $props();

let error = $state("");
let notice = $state("");
let busy = $state(false);
let importing = $state(false);
let jsonExport = $state<any>();
let session = 0;

function settingsSaved(settings: any) {
	if (data.inactivity.check()) return;
	data.lockSettings = settings;
}

async function downloadBackup(recovery = false) {
	if (busy || importing) return;
	busy = true;
	error = "";
	notice = "";
	const vault = data.vault;
	const request = session;
	try {
		const backup = await (recovery ? vault.exportTransitionBackup() : vault.exportBackup());
		if (data.inactivity.check() || request !== session) return;
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
	if (data.inactivity.check()) return;
	importing = false;
	notice = success ? "Import verified. All accepted items and folders were added." : "";
	const request = session;
	setTimeout(() => {
		if (request === session && !data.inactivity.check()) document.getElementById("import-json-action")?.focus();
	}, 0);
}

function showImport() {
	if (data.inactivity.check() || busy) return;
	importing = true;
	notice = "";
}
</script>

<div class="flex min-h-0 grow items-start justify-center overflow-auto p-6">
	<div class="space-y-6 max-w-3xl">
		{#if error}<Alert tone="danger">{error}</Alert>{/if}
		{#if notice}<p role="status">{notice}</p>{/if}
		<LockSettingsControl vault={data.vault} settings={data.lockSettings} onsaved={settingsSaved} />
		<JsonExport vault={data.vault} isActive={() => !data.inactivity.check()} />
		<ImportJson vault={data.vault} active={() => !data.inactivity.check()} oncancel={() => finishImport()} onimported={() => finishImport(true)} />
		<div class="flex gap-2">
			<button class="btn btn-secondary btn-md" type="button" disabled={busy || importing} onclick={() => void downloadBackup()}>{busy ? "Preparing backup…" : "Download encrypted backup"}</button>
			<button class="btn btn-secondary btn-md" type="button" disabled={busy || importing} onclick={() => void downloadBackup(true)}>Download pre-transition backup</button>
		</div>
	</div>
</div>
