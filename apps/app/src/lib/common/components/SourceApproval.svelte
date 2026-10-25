<script lang="ts">
    import { onMount } from "svelte";
    import {
        approvePendingNodeJoin,
        rejectPendingNodeJoin,
        pendingNodeJoins,
        nodeJoinAccepting,
        type PendingNodeJoin,
    } from "../state/devices.svelte";
    import { getDeviceApiUrl } from "../state/deviceClient.svelte";

    let {
        endpoint = null,
        qr = null,
        oncopy,
    }: {
        endpoint?: string | null;
        qr?: string | null;
        oncopy?: (endpoint: string) => void;
    } = $props();
    let nodeOffers = $state.raw<PendingNodeJoin[]>([]);
    let nodeAccepting = $state(false);
    let nodeError = $state<string | null>(null);
    let nodeStatus = $state("");
    let decisionError = $state<string | null>(null);
    let failedApproval = $state.raw<PendingNodeJoin | null>(null);
    let nodeBusy = $state(false);
    let nodeLoading = $state(true);
    let loaded = false;

    async function loadNodeOffers() {
        if (nodeLoading && loaded) return;
        loaded = true;
        nodeLoading = true;
        try {
            const [offers, accepting] = await Promise.all([
                pendingNodeJoins(),
                nodeJoinAccepting(),
            ]);
            nodeOffers = offers;
            nodeAccepting = accepting;
            nodeError = null;
        } catch (cause) {
            nodeError = String(cause);
        } finally {
            nodeLoading = false;
        }
    }

    async function setNodeAccepting(accepting: boolean) {
        nodeBusy = true;
        nodeError = null;
        nodeStatus = "";
        try {
            nodeAccepting = await nodeJoinAccepting(accepting);
            await loadNodeOffers();
        } catch (cause) {
            nodeError = String(cause);
        } finally {
            nodeBusy = false;
        }
    }

    async function decideNodeOffer(offer: PendingNodeJoin, approve: boolean) {
        if (
            approve &&
            !confirm(
                `Approve ${offer.request.name} as an equal Node in ${offer.request.installation}? Verify the authenticated endpoint and independent public signer shown below.`,
            )
        )
            return;
        nodeBusy = true;
        decisionError = null;
        nodeStatus = "";
        try {
            if (approve) {
                const result = await approvePendingNodeJoin(offer.offer_id);
                nodeStatus = result === "accepted"
                    ? "Approval accepted. The other device is still finishing enrollment and provisioning. Check progress there; this is not proof it is ready."
                    : "Source approval recorded. The other device must still finish provisioning and verify every owner before sign-in.";
            } else {
                await rejectPendingNodeJoin(offer.offer_id);
                nodeStatus = "Source rejection acknowledged; target readiness is not established.";
            }
            failedApproval = null;
            await loadNodeOffers();
        } catch (cause) {
            decisionError = String(cause);
            failedApproval = approve ? offer : null;
        } finally {
            nodeBusy = false;
        }
    }

    onMount(() => {
        void loadNodeOffers();
        const timer = setInterval(() => {
            if (!nodeBusy) void loadNodeOffers();
        }, 5000);
        return () => clearInterval(timer);
    });
</script>

<section class="flex flex-col gap-2" aria-labelledby="node-joining-heading">
    <h2 id="node-joining-heading">Add a device</h2>
    <ol class="list-decimal pl-6">
        <li>Allow requests here while you add your other device.</li>
        <li>On the other device, choose <strong>Join an existing Installation</strong> and use this trusted source endpoint.</li>
        <li>Compare the request below with the other device before approving. Stop accepting requests when finished.</li>
    </ol>
    {#if endpoint}
        <code class="break-all">{endpoint}</code>
        {#if oncopy}<button type="button" class="btn secondary" onclick={() => oncopy?.(endpoint)}>Copy endpoint ID</button>{/if}
        <details>
            <summary>Joining independently deployed owners?</summary>
            <p>Use this full trusted peer instead of the endpoint alone:</p>
            <code class="break-all">{getDeviceApiUrl()}#{endpoint}</code>
        </details>
        {#if qr}<img class="h-64 w-64 bg-white p-2" src={qr} alt="Trusted source endpoint QR code" />{/if}
    {:else}
        <p>Source endpoint unavailable. Refresh the Devices page before sharing a trusted peer.</p>
    {/if}
    <p>Approval enrolls a Node and its independent public signer, not general resource access. Existing permissions, restrictions, and data selections still apply.</p>
    <button
        type="button"
        class="btn secondary"
        disabled={nodeBusy || nodeLoading}
        onclick={() => void setNodeAccepting(!nodeAccepting)}
    >
        {nodeAccepting ? "Stop accepting requests" : "Allow requests"}
    </button>
    <button
        type="button"
        class="btn secondary"
        disabled={nodeBusy || nodeLoading}
        onclick={() => void loadNodeOffers()}>Refresh requests</button
    >
    {#if nodeError}<p role="alert">Could not load source requests. Refresh requests after signing in with permission to manage Devices. {nodeError}</p>{/if}
    {#if decisionError}
        <div role="alert">
            <p>The decision could not be confirmed. {decisionError}</p>
            {#if failedApproval}
                <p>Enrollment may already be partly applied. Fix the owner connection or permission, then retry only this exact request. Do not reset the joining device or create a replacement identity. No automatic rollback was attempted.</p>
                <p>Request {failedApproval.offer_id}: {failedApproval.request.name}</p>
                <button type="button" class="btn secondary" disabled={nodeBusy || nodeLoading} onclick={() => failedApproval && void decideNodeOffer(failedApproval, true)}>Retry this approval</button>
            {:else}
                <p>Refresh requests and retry rejection only if the same request is still pending.</p>
            {/if}
        </div>
    {/if}
    {#if !nodeLoading && !nodeError && nodeOffers.length === 0}<p>{nodeAccepting ? "Waiting for a request from your other device." : "Requests are closed. Allow requests to begin."}</p>{/if}
    {#if nodeStatus}<p role="status">{nodeStatus}</p>{/if}
    {#each nodeOffers as offer (offer.offer_id)}
        <article class="flex flex-col gap-2">
            <h3>{offer.request.name}</h3>
            <p>Verify these authenticated bindings with the intended device. These are not configuration choices.</p>
            <dl>
                <dt>Exact request</dt><dd class="break-all">{offer.offer_id}</dd>
                <dt>Installation</dt>
                <dd class="break-all">{offer.request.installation}</dd>
                <dt>Authenticated endpoint</dt>
                <dd class="break-all">{offer.authenticated_endpoint}</dd>
                {#if offer.request.device_id}
                    <dt>Device identity</dt><dd class="break-all">{offer.request.device_id}</dd>
                {/if}
                {#if offer.request.operation}
                    {#each ["idp", "device", "storage"] as owner (owner)}
                        <dt>Target {owner}</dt><dd class="break-all">{offer.request.operation[owner as "idp" | "device" | "storage"]}</dd>
                    {/each}
                {/if}
                <dt>Signer member</dt>
                <dd class="break-all">{offer.request.member_id}</dd>
                <dt>Approving endpoint</dt>
                <dd class="break-all">{offer.request.approving_peer}</dd>
                <dt>Independent public signer</dt>
                <dd>
                    <pre
                        class="whitespace-pre-wrap break-all">{JSON.stringify(
                            offer.request.public_signer,
                            null,
                            2,
                        )}</pre>
                </dd>
            </dl>
            <div class="flex gap-2">
                <button
                    type="button"
                    class="btn primary"
                    disabled={nodeBusy || nodeLoading}
                    onclick={() => void decideNodeOffer(offer, true)}
                    >Approve this device</button
                >
                <button
                    type="button"
                    class="btn secondary"
                    disabled={nodeBusy || nodeLoading}
                    onclick={() => void decideNodeOffer(offer, false)}
                    >Reject</button
                >
            </div>
        </article>
    {/each}
</section>
