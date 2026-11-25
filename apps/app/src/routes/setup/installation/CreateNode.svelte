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
        nodeSetupCode,
        nodeSetupStatus,
        readNodeCreateIntent,
    } from "$lib/common/state/nodeSetup";
    import { Alert, Card, Field, Page, PageHeader } from "$lib/ui";

    let code = $state("");
    let administrator = $state("");
    let password = $state("");
    let intent = $state.raw<NodeCreateIntent | null>();
    let busy = $state(false);
    let waiting = $state(false);
    let error = $state("");
    let mounted = false;

    onMount(() => {
        mounted = true;
        code = nodeSetupCode("create");
        if (code) void run(inspect);
        return () => {
            mounted = false;
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
            waiting = false;
        }
    }

    async function inspect() {
        intent = undefined;
        password = "";
        const status = await nodeSetupStatus();
        if (!mounted) return;
        if (!status || status.state === "unavailable")
            throw new Error(
                "Cannot verify Node setup. Check the connection and try again.",
            );
        if (status.state !== "creating" && status.state !== "blocked") {
            await goto(resolve(status.route), { replaceState: true });
            return;
        }
        if (code) intent = await readNodeCreateIntent(code);
    }

    async function checkReadiness() {
        waiting = true;
        const deadline = Date.now() + 30_000;
        while (mounted && Date.now() < deadline) {
            const status = await nodeSetupStatus();
            if (!mounted) return;
            if (
                status &&
                (status.route === "/signin" ||
                    status.route === "/setup/join" ||
                    status.state === "awaiting_choice")
            ) {
                await goto(resolve(status.route), { replaceState: true });
                return;
            }
            if (status?.state === "blocked") break;
            await new Promise((resolve) => setTimeout(resolve, 500));
        }
        if (!mounted) return;
        waiting = false;
        error =
            "The Node is not ready yet. Check retained setup before retrying.";
        await inspect();
    }

    async function mutate(
        operation: () => Promise<void>,
        after: () => Promise<void>,
    ) {
        intent = undefined;
        try {
            await operation();
        } catch (cause) {
            report(cause);
            if (mounted) await inspect();
            return;
        }
        if (mounted) await after();
    }

    function submit(event: SubmitEvent) {
        event.preventDefault();
        if (intent === undefined) return run(inspect);
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
                checkReadiness,
            ),
        );
    }

    function cancel() {
        const retained = intent;
        if (!retained) return;
        return run(() =>
            mutate(async () => {
                await cancelNodeCreate(code, retained.operation_nonce);
                administrator = "";
            }, inspect),
        );
    }
</script>

<Page width="narrow">
    <PageHeader title="Create an Installation" />
    <Card>
        {#if error}<Alert tone="danger">{error}</Alert>{/if}
        {#if waiting}
            <Alert tone="info">Waiting for the Node to be ready.</Alert>
        {:else if busy}
            <Alert tone="info">Checking Node setup.</Alert>
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
                                onclick={() => run(inspect)}
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
