<script lang="ts">
import type { FileSystemResource } from "@aicacia/storage-client";
import type { StorageAccess } from "@of/sdk";
import { flushSync, onMount } from "svelte";
import { goto } from "$app/navigation";
import { resolve } from "$app/paths";
import SecretEditor from "$lib/components/secrets/SecretEditor.svelte";
import { notifications } from "$lib/common/state/notifications.svelte";
import { getSdk } from "$lib/common/state/sdk";
import type { PageProps } from "./$types";

let { data }: PageProps = $props();
const sdk = getSdk();
let session = $state.raw(sdk.session);
let context = $state.raw<{
    client: StorageAccess;
    userId: string;
    signal: AbortSignal;
} | null>(null);
let fileSystems = $state.raw<FileSystemResource[]>([]);
let selectedFileSystemId = $state("");
let openVersion = $state(0);
let loading = $state(true);
let error = $state<string | null>(null);

onMount(() =>
    sdk.subscribe((next) => {
        const wasAuthenticated = session.status === "authenticated";
        session = next;
        if (
            (wasAuthenticated && next.status !== "authenticated") ||
            next.status === "ended" ||
            next.status === "disposed"
        )
            flushSync();
    }),
);

$effect(() => {
    const subject = session.identity?.subject;
    if (
        session.status !== "authenticated" ||
        !subject ||
        subject !== data.userInfo?.sub
    )
        return;
    const controller = new AbortController();
    let active = true;
    fileSystems = [];
    selectedFileSystemId = "";
    loading = true;
    error = null;
    try {
        const client = getSdk().storage();
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

async function signOut(): Promise<void> {
    try {
        await sdk.signout();
    } catch {
        notifications.add(
            "Local credential removal failed. Clear browser site data before signing in again.",
        );
    }
    await goto(resolve("/signin"));
}

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

{#if session.status === 'authenticated' && session.identity?.subject === data.userInfo?.sub}
    <button class="btn secondary" type="button" onclick={() => void signOut()}>Sign out</button>
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
                    <SecretEditor client={context.client} userId={context.userId} filesystemId={selectedFileSystemId} />
                {/key}
            {:else}
                <p>Select or create a filesystem to view secrets.</p>
            {/if}
        {/key}
    </div>
{:else if session.status === 'ended' || session.status === 'disposed' || session.status === 'unauthenticated'}
    <p>Your session has ended. <a href={resolve('/signin')}>Sign in again</a></p>
{/if}
