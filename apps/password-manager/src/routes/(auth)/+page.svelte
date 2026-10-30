<script lang="ts">
import type { StorageAccess } from "@of/sdk";
import { flushSync, onMount } from "svelte";
import { goto } from "$app/navigation";
import { resolve } from "$app/paths";
import SecretEditor from "$lib/components/secrets/SecretEditor.svelte";
import { notifications } from "$lib/common/state/notifications.svelte";
import { getSdk } from "$lib/common/state/sdk";
import { prepareSecrets } from "$lib/storage/startup";
import { storageMessage } from "$lib/storage/sql";
import type { PageProps } from "./$types";

let { data }: PageProps = $props();
const sdk = getSdk();
let session = $state.raw(sdk.session);
let context = $state.raw<{ client: StorageAccess; databaseId: string } | null>(
    null,
);
let preparing = $state(true);
let error = $state<string | null>(null);
let controller: AbortController | undefined;

function retire() {
    controller?.abort();
    controller = undefined;
    context = null;
}

async function prepare() {
    retire();
    if (
        session.status !== "authenticated" ||
        session.identity?.subject !== data.userInfo?.sub
    )
        return;
    const current = new AbortController();
    controller = current;
    preparing = true;
    error = null;
    try {
        const client = sdk.storage();
        const database = await prepareSecrets(client, current.signal);
        if (controller === current)
            context = { client, databaseId: database.id };
    } catch (cause) {
        if (controller === current)
            error =
                cause instanceof Error &&
                /^(Incompatible|Malformed)/.test(cause.message)
                    ? cause.message
                    : storageMessage(cause);
    } finally {
        if (controller === current) preparing = false;
    }
}

onMount(() => {
    let started = false;
    const unsubscribe = sdk.subscribe((next) => {
        const changed =
            next.generation !== session.generation ||
            next.status !== session.status;
        session = next;
        if (!started || changed) {
            started = true;
            void prepare();
            if (changed) flushSync();
        }
    });
    return () => {
        unsubscribe();
        retire();
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
</script>

{#if session.status === 'authenticated' && session.identity?.subject === data.userInfo?.sub}
    <button class="btn secondary" type="button" onclick={() => void signOut()}>Sign out</button>
    <div class="mx-auto flex w-full max-w-5xl grow flex-col gap-6 overflow-auto p-6">
        <header>
            <h1>Password Manager</h1>
            <p>Your User/Application database opens automatically. This example adds no end-to-end encryption or concurrent-edit protection. Use test secrets only.</p>
        </header>
        {#if error}
            <p role="alert">{error}</p>
            <button class="btn secondary" type="button" onclick={() => void prepare()}>Retry preparation</button>
        {:else if preparing}
            <p role="status">Preparing secrets database…</p>
        {/if}
        {#key context}
            {#if context}
                <SecretEditor client={context.client} databaseId={context.databaseId} />
            {/if}
        {/key}
    </div>
{:else if session.status === 'ended' || session.status === 'disposed' || session.status === 'unauthenticated'}
    <p>Your session has ended. <a href={resolve('/signin')}>Sign in again</a></p>
{/if}
