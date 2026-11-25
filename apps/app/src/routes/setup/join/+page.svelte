<script lang="ts">
    import type { OidcClient } from "@ofsdk/oidc-client";
    import { onMount } from "svelte";
    import { replaceState } from "$app/navigation";
    import { resolve } from "$app/paths";
    import { clearNodeSetupCode, nodeSetupCode, unifiedNode } from "$lib/common/state/nodeSetup";
    import { Alert, Card, Field, Page, PageHeader } from "$lib/ui";
    import { type JoinIntent, type JoinView, joinAction, joinClient, joinStatus } from "./joinApi";

    const storageKey = "node-join-v1";
    const fields: [keyof JoinIntent, string][] = [
        ["issuer", "Installation issuer"],
        ["device", "Device API"],
        ["storage", "Storage API"],
        ["clientId", "Join OAuth client ID"],
        ["application", "Join Application URI"],
        ["acceptingPublicKey", "Accepting Device public key"],
        ["name", "Joining Device name"],
        ["address", "Joining Device address"],
    ];
    let input = $state<JoinIntent>({
        issuer: "", device: "", storage: "", clientId: "", application: "",
        acceptingPublicKey: "", name: "", address: "",
    });
    const node = unifiedNode();
    let helper = $state(node ?? "");
    let code = $state("");
    let usingHandoff = $state(false);
    let view = $state.raw<JoinView>({ stage: "setup" });
    let ready = $state(false);
    let busy = $state(false);
    let error = $state("");
    let signedIn = $state(false);
    let client: OidcClient | undefined;

    async function run(operation: () => Promise<void>) {
        if (busy) return;
        busy = true;
        error = "";
        try {
            await operation();
        } catch (cause) {
            error = cause instanceof Error ? cause.message : "Join could not continue.";
        } finally {
            busy = false;
        }
    }

    function remember(next: JoinView) {
        if (next.stage === "approved") {
            client?.clearStoredTokenResponse();
            signedIn = false;
            clearNodeSetupCode("join", code);
            code = "";
        }
        if (next.stage !== "setup") sessionStorage.setItem(storageKey, JSON.stringify({ helper, view: next }));
        view = next;
    }

    async function signInClient(intent: JoinIntent) {
        return joinClient(intent, new URL(resolve("/setup/join"), location.origin).href);
    }

    async function resumeSignIn(intent: JoinIntent, callback: boolean) {
        signedIn = false;
        try {
            client = await signInClient(intent);
            if (callback) await client.handleSigninCallback();
            if (client.getStoredTokenResponse()?.access_token) {
                await client.getVerifiedIdentity();
                signedIn = true;
            }
        } catch {
            client?.clearStoredTokenResponse();
            throw new Error("Sign-in could not be verified against this Installation. Sign in again.");
        } finally {
            if (callback) replaceState(resolve("/setup/join"), {});
        }
    }

    onMount(() => {
        if (location.protocol !== "https:") {
            error = "Join requires an HTTPS app origin.";
            return;
        }
        code = nodeSetupCode("join");
        usingHandoff = Boolean(code);
        ready = true;
        void run(async () => {
            const raw = sessionStorage.getItem(storageKey);
            const saved = raw ? JSON.parse(raw) as { helper: string; view: JoinView } : undefined;
            helper = node ?? saved?.helper ?? "";
            if (!helper) return;
            const current = await joinStatus(helper);
            if (current.stage === "approved") {
                remember(current);
                return;
            }
            if (saved?.helper !== helper || (saved.view.stage !== "prepared" && saved.view.stage !== "pending")) return;
            const params = new URLSearchParams(location.search);
            view = saved.view;
            await resumeSignIn(view.intent, params.has("code") || params.has("error"));
        });
    });

    async function prepare() {
        await run(async () => remember(await joinAction(helper, code, { action: "prepare", intent: $state.snapshot(input) })));
    }

    async function resume() {
        await run(async () => {
            const current = await joinStatus(helper);
            const next = current.stage === "approved" ? current : await joinAction(helper, code, { action: "resume" });
            remember(next);
            if (next.stage === "prepared" || next.stage === "pending") await resumeSignIn(next.intent, false);
        });
    }

    async function signIn() {
        if (view.stage !== "prepared" && view.stage !== "pending") return;
        const { intent } = view;
        await run(async () => {
            if (!node) code = "";
            try {
                client = await signInClient(intent);
                await client.signin();
            } catch {
                throw new Error("Cannot start sign-in with the selected Installation.");
            }
        });
    }

    async function pair(action: "request" | "approve") {
        await run(async () => {
            if (!client || !signedIn) return;
            let accessToken: string;
            try {
                accessToken = await client.getAccessToken();
            } catch {
                signedIn = false;
                throw new Error("Your Installation session expired. Sign in again.");
            }
            remember(await joinAction(helper, code, { action, accessToken }));
        });
    }
</script>

<Page width="narrow">
    <PageHeader title="Join an existing Installation" />
    <Card>
        {#if error}<Alert tone="danger">{error}</Alert>{/if}
        {#if view.stage === "approved"}
            <Alert tone="success">This Node joined as a Device.</Alert>
            <dl>
                <dt>Device record ID</dt><dd><code>{view.deviceId}</code></dd>
                <dt>Endpoint public key</dt><dd><code class="break-all">{view.publicKey}</code></dd>
            </dl>
            <p>Approval does not grant Resource access or select Resources.</p>
        {:else if ready}
            <form onsubmit={(event) => { event.preventDefault(); if (view.stage === "setup") void prepare(); }}>
                {#if !node}<Field label="Local join helper">
                    {#snippet children({ id })}
                        <input {id} type="url" bind:value={helper} disabled={busy || view.stage !== "setup"} class="input" />
                    {/snippet}
                </Field>{/if}
                {#if !usingHandoff}
                    <Field label={node ? "Node setup code" : "Local setup code"}>
                        {#snippet children({ id })}
                            <input {id} type="password" bind:value={code} disabled={busy} autocomplete="off" class="input" />
                        {/snippet}
                    </Field>
                {:else if error}
                    <button type="button" class="btn btn-secondary btn-md" disabled={busy} onclick={() => {
                        clearNodeSetupCode("join", code);
                        code = "";
                        usingHandoff = false;
                    }}>Enter Node setup code</button>
                {/if}
                {#if view.stage === "setup"}
                    {#each fields as [key, label] (key)}
                        <Field {label}>
                            {#snippet children({ id })}
                                <input {id} bind:value={input[key]} disabled={busy} class="input" />
                            {/snippet}
                        </Field>
                    {/each}
                    <button type="submit" class="btn btn-primary btn-md" disabled={busy}>Prepare join</button>
                {:else}
                    <p>Selected Installation: <strong class="break-all">{view.intent.issuer}</strong></p>
                    {#if view.stage === "pending"}
                        <Alert tone="info">Pairing is pending.</Alert>
                        <dl>
                            <dt>Joining Device</dt><dd>{view.intent.name}</dd>
                            <dt>Device record ID</dt><dd><code>{view.deviceId}</code></dd>
                            <dt>Endpoint public key</dt><dd><code class="break-all">{view.publicKey}</code></dd>
                            <dt>Accepting Device public key</dt><dd><code class="break-all">{view.intent.acceptingPublicKey}</code></dd>
                        </dl>
                    {/if}
                    {#if signedIn}
                        <p>Signed in to the Installation.</p>
                        {#if view.stage === "prepared"}
                            <button type="button" class="btn btn-primary btn-md" disabled={busy} onclick={() => pair("request")}>Request pairing</button>
                        {:else}
                            <button type="button" class="btn btn-primary btn-md" disabled={busy} onclick={() => pair("approve")}>Approve this Device</button>
                        {/if}
                    {:else}
                        <button type="button" class="btn btn-primary btn-md" disabled={busy} onclick={signIn}>Sign in to Installation</button>
                    {/if}
                {/if}
                <button type="button" class="btn btn-secondary btn-md" disabled={busy || !helper} onclick={resume}>Resume join</button>
            </form>
        {/if}
    </Card>
</Page>
