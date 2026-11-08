<script lang="ts">
import { tick } from "svelte";
import type { createLocalSecretWorkflow } from "../workflow.svelte";
let { workflow, navigate }: { workflow: ReturnType<typeof createLocalSecretWorkflow>; navigate: (action: () => void) => void } = $props();
let form = $state<{ action: "create" | "rename" | "delete"; id: string; name: string } | null>(null);
let destination = $state("");
const selected = $derived(workflow.folders.find((folder) => folder.id === workflow.folderFilter));
export function dirty() { return form !== null; }
export function retire() { form = null; destination = ""; }
function openForm(value: NonNullable<typeof form>) {
    navigate(() => { workflow.cancel(); form = value; });
}
async function save() {
    if (!form) return;
    const value = form;
    if (value.action === "create") await workflow.createFolder(value.name);
    else if (value.action === "rename") await workflow.renameFolder(value.id, value.name);
    else await workflow.deleteFolder(value.id);
    if (!workflow.error) await closeForm();
}
async function closeForm() {
    form = null;
    await tick();
}</script>

<section class="flex flex-col gap-3" aria-label="Folders">
    <p>Names are literal. Slashes do not create a hierarchy. Duplicate names keep separate IDs.</p>
    <div class="flex flex-wrap gap-2">
        <button class="btn btn-secondary btn-md" type="button" disabled={!workflow.canEdit}
            onclick={() => openForm({ action: "create", id: "", name: "" })}>New folder</button>
        <button class="btn btn-secondary btn-md" type="button" disabled={!workflow.canEdit || !selected}
            onclick={() => { const folder = selected; if (folder) openForm({ action: "rename", ...folder }); }}>Rename folder</button>
        <button class="btn btn-secondary btn-md" type="button" disabled={!workflow.canEdit || !selected}
            onclick={() => { const folder = selected; if (folder) openForm({ action: "delete", ...folder }); }}>Delete folder</button>
    </div>
    {#if form}
        {@const current = form}
        <form class="flex flex-col gap-2" onsubmit={(event) => { event.preventDefault(); void save(); }}>
            {#if current.action === "delete"}
                <p>Delete folder {current.name}? Its direct items, including Trash, will become unfiled. No items or other folders will be deleted.</p>
            {:else}
                <label class="flex flex-col gap-1">Folder name
                    <input aria-label="Folder name" required bind:value={current.name} disabled={!workflow.canEdit} {@attach (element) => { element.focus(); }} />
                </label>
            {/if}
            <div class="flex gap-2">
                <button class="btn btn-primary btn-md" type="submit" disabled={!workflow.canEdit}
                    {@attach (element) => { if (current.action === "delete") element.focus(); }}>{current.action === "delete" ? "Confirm folder delete" : "Save folder"}</button>
                <button class="btn btn-secondary btn-md" type="button" disabled={!workflow.canEdit} onclick={() => { void closeForm(); }}>Cancel folder</button>
            </div>
        </form>
    {/if}
    {#if workflow.detail && !workflow.editor}
        {#key workflow.detail}
            <div class="flex flex-wrap items-end gap-2">
                <label class="flex flex-col gap-1">Move to folder
                    <select aria-label="Move to folder" bind:value={destination} disabled={!workflow.canEdit}>
                        <option value="">Unfiled</option>
                        {#each workflow.folders as folder (folder.id)}
                            <option value={folder.id}>{folder.name} — {folder.id}</option>
                        {/each}
                    </select>
                </label>
                <button class="btn btn-secondary btn-md" type="button" disabled={!workflow.canEdit}
                    onclick={() => { const id = workflow.detail?.id; const folderId = destination || null; if (id) navigate(() => { void workflow.moveItem(id, folderId); }); }}>Move item</button>
            </div>
        {/key}
    {/if}
</section>
