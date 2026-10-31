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
import { Alert, Page, PageHeader } from "$lib/ui";
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
    <div class="min-h-0 grow overflow-auto">
        <Page>
            <PageHeader
                title="Password Manager"
                description="Your User/Application database opens automatically. This example adds no end-to-end encryption or concurrent-edit protection. Use test secrets only."
            >
                {#snippet actions()}
                    <button
                        class="btn btn-secondary btn-md"
                        type="button"
                        onclick={() => void signOut()}>Sign out</button
                    >
                {/snippet}
            </PageHeader>

            {#if error}
                <Alert tone="danger" title="Preparation failed">{error}</Alert>
                <div>
                    <button
                        class="btn btn-secondary btn-md"
                        type="button"
                        onclick={() => void prepare()}>Retry preparation</button
                    >
                </div>
            {:else if preparing}
                <Alert tone="info" title="Preparing secrets database…"
                    >Opening your vault.</Alert
                >
            {/if}

            {#key context}
                {#if context}
                    <SecretEditor
                        client={context.client}
                        databaseId={context.databaseId}
                    />
                {/if}
            {/key}
        </Page>
    </div>
{:else if session.status === 'ended' || session.status === 'disposed' || session.status === 'unauthenticated'}
    <div class="min-h-0 grow overflow-auto">
        <Page width="narrow">
            <Alert tone="warning">
                Your session has ended.
                <a href={resolve('/signin')}>Sign in again</a>
            </Alert>
        </Page>
    </div>
{/if}
