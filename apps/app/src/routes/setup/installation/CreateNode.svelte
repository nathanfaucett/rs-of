<script lang="ts">
    import { onMount } from "svelte";
    import { goto } from "$app/navigation";
    import { resolve } from "$app/paths";
    import {
        cancelNodeCreate,
        clearNodeSetupCode,
        createNode,
        type NodeCreateIntent,
        NodeSetupError,
        type NodeSetupStatus,
        nodeSetupCode,
        nodeSetupStatus,
        readNodeCreateIntent,
        watchNodeSetupStatus,
    } from "$lib/common/state/nodeSetup";
    import { Alert, Card, Field, Page, PageHeader } from "$lib/ui";

    let code = $state("");
    let administrator = $state("");
    let password = $state("");
    let intent = $state.raw<NodeCreateIntent | null>();
    let busy = $state(false);
    let waiting = $state(false);
    let needsCheck = $state(false);
    let error = $state("");
    let stopWatching: (() => void) | undefined;
    let mounted = false;

    onMount(() => {
        mounted = true;
        code = nodeSetupCode("create");
        if (code) void checkStatus();
        return () => {
            mounted = false;
            stopWatching?.();
        };
    });

    function report(cause: unknown) {
        if (cause instanceof NodeSetupError && cause.status === 401) {
            clearNodeSetupCode("create", code);
            code = "";
        }
        error =
            cause instanceof Error
                ? cause.message
                : "Cannot continue Node setup.";
    }

    async function run(operation: () => Promise<void>) {
        if (busy) return;
        busy = true;
        error = "";
        try {
            await operation();
        } catch (cause) {
            report(cause);
        } finally {
            busy = false;
        }
    }

    async function applyStatus(status: NodeSetupStatus | undefined) {
        if (!status || status.state === "unavailable") {
            needsCheck = true;
            throw new Error(
                "Cannot verify Node setup. Check the connection and try again.",
            );
        }
        needsCheck = false;
        if (status.state !== "creating" && status.state !== "blocked") {
            await goto(resolve(status.route), { replaceState: true });
            return;
        }
        intent = undefined;
        if (code) intent = await readNodeCreateIntent(code);
        if (!mounted) return;
        if (status.state === "creating" && status.active && code) watch();
        if (status.state === "blocked" && !error) {
            error =
                "The Node reports that setup is blocked. Check the server terminal.";
        }
    }

    async function inspect() {
        stopWatching?.();
        waiting = false;
        password = "";
        const status = await nodeSetupStatus();
        if (mounted) await applyStatus(status);
    }

    function checkStatus() {
        return run(inspect);
    }

    function watch() {
        stopWatching?.();
        waiting = true;
        stopWatching = watchNodeSetupStatus(
            (status) => {
                if (status.state === "creating" && status.active) return;
                stopWatching?.();
                waiting = false;
                void run(() => applyStatus(status));
            },
            () => {
                waiting = false;
                needsCheck = true;
                error =
                    "Node status updates stopped. Select Check Node status to reconnect.";
            },
        );
    }

    async function mutate(
        operation: () => Promise<void>,
        after: () => Promise<void>,
    ) {
        try {
            await operation();
        } catch (cause) {
            report(cause);
            if (mounted) await inspect();
            return;
        }
        if (mounted) await after();
    }

    async function waitForCreate() {
        intent = undefined;
        try {
            intent = await readNodeCreateIntent(code);
        } catch (cause) {
            report(cause);
        }
        if (mounted && code) watch();
    }

    function submit(event: SubmitEvent) {
        event.preventDefault();
        if (waiting || needsCheck) return;
        if (intent === undefined) return checkStatus();
        const retained = intent;
        const entered = password;
        password = "";
        return run(() =>
            mutate(
                () =>
                    createNode(
                        code,
                        retained?.administrator ?? administrator,
                        retained?.password_required === false
                            ? undefined
                            : entered,
                    ),
                waitForCreate,
            ),
        );
    }

    function cancel() {
        const retained = intent;
        if (!retained) return;
        return run(async () => {
            stopWatching?.();
            waiting = false;
            await mutate(async () => {
                await cancelNodeCreate(code, retained.operation_nonce);
                administrator = "";
            }, inspect);
        });
    }
</script>

<Page width="narrow">
    <PageHeader title="Create an Installation" />
    <Card>
        {#if error}<Alert tone="danger">{error}</Alert>{/if}
        {#if busy}
            <Alert tone="info">Checking Node setup.</Alert>
        {:else if waiting || needsCheck}
            {#if waiting}
                <Alert tone="info">Waiting for the Node to be ready.</Alert>
            {/if}
            <div class="flex flex-wrap gap-4">
                {#if intent}
                    <button
                        type="button"
                        class="btn btn-secondary btn-md"
                        onclick={cancel}
                    >
                        Cancel Create
                    </button>
                {/if}
                <button
                    type="button"
                    class="btn btn-secondary btn-md"
                    onclick={checkStatus}
                >
                    Check Node status
                </button>
            </div>
        {:else}
            <form onsubmit={submit} class="flex flex-col gap-4">
                {#if intent === undefined}
                    <p>
                        Use the same Node setup code from the file shown in the
                        server terminal.
                    </p>
                    <Field label="Node setup code">
                        {#snippet children({ id })}
                            <input
                                {id}
                                type="password"
                                bind:value={code}
                                autocomplete="off"
                                required
                                class="input"
                            />
                        {/snippet}
                    </Field>
                    <button type="submit" class="btn btn-primary btn-md">
                        Continue
                    </button>
                {:else}
                    {#if intent}
                        <Alert tone="info">
                            A Create request is retained. Retry with the
                            original details or cancel this request.
                        </Alert>
                    {/if}
                    <Field label="Administrator username">
                        {#snippet children({ id })}
                            {#if intent}
                                <input
                                    {id}
                                    value={intent.administrator}
                                    readonly
                                    class="input"
                                />
                            {:else}
                                <input
                                    {id}
                                    bind:value={administrator}
                                    autocomplete="username"
                                    required
                                    class="input"
                                />
                            {/if}
                        {/snippet}
                    </Field>
                    {#if !intent || intent.password_required}
                        <Field label="Administrator password">
                            {#snippet children({ id })}
                                <input
                                    {id}
                                    type="password"
                                    bind:value={password}
                                    autocomplete="new-password"
                                    required
                                    class="input"
                                />
                            {/snippet}
                        </Field>
                        {#if intent}<p>
                                Enter the original administrator password.
                            </p>{/if}
                    {/if}
                    <div class="flex flex-wrap gap-4">
                        <button type="submit" class="btn btn-primary btn-md">
                            {intent ? "Retry Create" : "Create"}
                        </button>
                        {#if intent}
                            <button
                                type="button"
                                class="btn btn-secondary btn-md"
                                onclick={cancel}
                            >
                                Cancel Create
                            </button>
                            <button
                                type="button"
                                class="btn btn-secondary btn-md"
                                onclick={checkStatus}
                            >
                                Check Node status
                            </button>
                        {/if}
                    </div>
                {/if}
            </form>
        {/if}
    </Card>
</Page>
