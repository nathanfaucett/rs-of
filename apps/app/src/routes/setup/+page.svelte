<script lang="ts">
    import { onMount } from "svelte";
    import { goto } from "$app/navigation";
    import { resolve } from "$app/paths";
    import {
        chooseNodeSetup,
        type NodeSetupStatus,
        nodeSetupStatus,
        watchNodeSetupStatus,
    } from "$lib/common/state/nodeSetup";
    import { Alert, Card, Field, Page, PageHeader } from "$lib/ui";

    import type { PageProps } from "./$types";

    let { data }: PageProps = $props();
    let code = $state("");
    let busy = $state(false);
    let error = $state("");

    let setupState = $derived(data.state);
    let stopWatching: (() => void) | undefined;
    let mounted = false;

    onMount(() => {
        mounted = true;
        if (setupState === "starting") watch();
        return () => {
            mounted = false;
            stopWatching?.();
        };
    });

    async function applyStatus(status: NodeSetupStatus) {
        if (status.route === "/setup") setupState = status.state;
        else await goto(resolve(status.route), { replaceState: true });
    }

    function watch() {
        stopWatching?.();
        stopWatching = watchNodeSetupStatus(
            (status) => {
                if (status.state === "starting") return;
                stopWatching?.();
                void run(() => applyStatus(status));
            },
            () => {
                error =
                    "Node status updates stopped. Select Check Node status to reconnect.";
            },
        );
    }

    async function run(operation: () => Promise<void>) {
        if (busy) return;
        busy = true;
        error = "";
        try {
            await operation();
        } catch (cause) {
            error =
                cause instanceof Error
                    ? cause.message
                    : "Cannot verify Node setup.";
        } finally {
            busy = false;
        }
    }

    function choose(choice: "create" | "join") {
        return run(async () => {
            await chooseNodeSetup(choice, code);
            if (!mounted) return;
            code = "";
            setupState = "starting";
            watch();
        });
    }

    function checkStatus() {
        return run(async () => {
            stopWatching?.();
            const status = await nodeSetupStatus();
            if (!mounted) return;
            if (!status) {
                setupState = "unavailable";
                return;
            }
            await applyStatus(status);
            if (mounted && status.state === "starting") watch();
        });
    }
</script>

<Page width="narrow">
    <PageHeader title="Set up this Node" />
    <Card>
        <div class="flex flex-col gap-4">
            {#if error}<Alert tone="danger">{error}</Alert>{/if}
            {#if setupState === "starting"}
                <p>Your choice was saved. Starting the selected setup.</p>
            {:else if setupState === "unavailable" || setupState === "blocked"}
                <Alert tone="danger">
                    Cannot verify Node setup. Check the server terminal and
                    HTTPS trust.
                </Alert>
            {:else}
                <p>
                    Use the protected Node setup code from the file shown in the
                    server terminal.
                </p>
                <Field label="Node setup code">
                    {#snippet children({ id })}
                        <input
                            {id}
                            type="password"
                            bind:value={code}
                            autocomplete="off"
                            disabled={busy}
                            class="input"
                        />
                    {/snippet}
                </Field>
                <div class="flex flex-wrap gap-4">
                    <button
                        type="button"
                        class="btn btn-primary btn-md"
                        disabled={busy}
                        onclick={() => choose("create")}
                    >
                        Start a new Installation
                    </button>
                    <button
                        type="button"
                        class="btn btn-secondary btn-md"
                        disabled={busy}
                        onclick={() => choose("join")}
                    >
                        Join an existing Installation
                    </button>
                </div>
            {/if}
            <button
                type="button"
                class="btn btn-secondary btn-md"
                disabled={busy}
                onclick={checkStatus}
            >
                Check Node status
            </button>
        </div>
    </Card>
</Page>
