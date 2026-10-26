<script lang="ts">
import type {
    FileSystemResource,
    StorageClient,
} from "@aicacia/storage-client";
import { createStorageClient } from "$lib/common/storageSocket";
import Filesystem from "./Filesystem.svelte";

let { userId }: { userId: string } = $props();
let context = $state.raw<{
    client: StorageClient;
    userId: string;
    signal: AbortSignal;
} | null>(null);
let fileSystems = $state.raw<FileSystemResource[]>([]);
let selectedFileSystemId = $state("");
let openVersion = $state(0);
let loading = $state(true);
let error = $state<string | null>(null);

$effect(() => {
    const subject = userId;
    const controller = new AbortController();
    let active = true;
    fileSystems = [];
    selectedFileSystemId = "";
    loading = true;
    error = null;
    try {
        const client = createStorageClient();
        context = { client, userId: subject, signal: controller.signal };
        void client
            .listFileSystems(controller.signal)
            .then((resources) => {
                if (active) fileSystems = resources;
            })
            .catch((cause: unknown) => {
                if (active) error = message(cause);
            })
            .finally(() => {
                if (active) loading = false;
            });
    } catch (cause) {
        context = null;
        error = message(cause);
        loading = false;
    }
    return () => {
        active = false;
        controller.abort();
        context = null;
    };
});

async function newFileSystem(): Promise<void> {
    const current = context;
    if (!current) return;
    loading = true;
    error = null;
    try {
        const resource = await current.client.createFileSystem(
            "Password Manager",
            current.signal,
        );
        if (current !== context) return;
        fileSystems = [...fileSystems, resource];
        selectedFileSystemId = resource.id;
    } catch (cause) {
        if (current === context) error = message(cause);
    } finally {
        if (current === context) loading = false;
    }
}

function message(cause: unknown): string {
    return cause instanceof Error ? cause.message : "Storage request failed";
}
</script>

<div class="mx-auto flex w-full max-w-5xl grow flex-col gap-6 overflow-auto p-6">
    <header>
        <h1>Password Manager</h1>
        <p>Records belong to your User and Application namespace. Writes persist to Storage; switching cannot cancel a write already sent.</p>
    </header>
    <section class="card flex flex-wrap items-end gap-3">
        <label for="filesystem">Filesystem Storage Resource
            <select id="filesystem" bind:value={selectedFileSystemId} disabled={loading}>
                <option value="">Select a filesystem</option>
                {#each fileSystems as fileSystem (fileSystem.id)}
                    <option value={fileSystem.id}>{fileSystem.name ?? 'Unnamed filesystem'} ({fileSystem.id})</option>
                {/each}
            </select>
        </label>
        <button class="btn secondary" type="button" disabled={loading || !selectedFileSystemId} onclick={() => { openVersion += 1; }}>Open filesystem</button>
        <button class="btn secondary" type="button" disabled={loading || !context} onclick={() => void newFileSystem()}>Create filesystem</button>
    </section>
    {#if error}
        <p role="alert">{error}</p>
    {:else if loading}
        <p role="status">Loading filesystem Storage Resources…</p>
    {/if}
    {#key context}
        {#if context && selectedFileSystemId}
            {#key `${selectedFileSystemId}:${openVersion}`}
                <Filesystem client={context.client} userId={context.userId} filesystemId={selectedFileSystemId} />
            {/key}
        {:else}
            <p>Select or create a filesystem to view secrets.</p>
        {/if}
    {/key}
</div>
