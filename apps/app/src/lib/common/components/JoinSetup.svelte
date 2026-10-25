<script lang="ts">
    import { onDestroy, untrack } from "svelte";
    import { isTauri } from "@tauri-apps/api/core";
    import { env } from "$env/dynamic/public";
    import { getIdpApiUrl } from "../state/idpClient.svelte";
    import { getDeviceApiUrl } from "../state/deviceClient.svelte";
    import { setupBase } from "../state/setupApi";
    import { resolve } from "$app/paths";
    import { WebSetupCreate, readNativeSetupCode, isUnifiedSetupHost, usesRemoteApis } from "../state/setupClient.svelte";
    import { observeOnboarding, type OnboardingObservation } from "../state/onboarding";

    let { observation: supplied, onworking }: { observation: OnboardingObservation; onworking?: () => void } = $props();
    const initial = untrack(() => supplied);
    let observation = $state.raw(initial);
    const host = initial.hosts[0];
    const session = new WebSetupCreate(host);
    let code = $state("");
    let peer = $state("");
    let busy = $state(false);
    let error = $state("");
    let active = true;
    const exact = $derived(session.intent && "peer" in session.intent ? session.intent : null);
    const working = $derived(!!session.intent || ["joining", "unfinished"].includes(session.status?.state ?? ""));
    const ready = $derived(observation.state === "ready" && session.status?.ready && !session.observationRequired);
    const permitted = $derived(isUnifiedSetupHost(host) && observation.state !== "starting" && (observation.state !== "blocked" || (observation.action === "repair" && session.status?.state === "unfinished" && !!exact)));
    const canJoin = $derived(permitted && !ready && !session.observationRequired && (exact || session.status?.state === "awaiting_choice"));

    $effect(() => {
        if (!working || ready || observation.state === "blocked") return;
        const timer = setInterval(() => void refresh(), 3000);
        return () => clearInterval(timer);
    });
    onDestroy(() => { active = false; code = ""; session.destroy(); });

    function requireTarget() {
        const bundled = isTauri() && !usesRemoteApis();
        const selected = getIdpApiUrl();
        if (
            bundled !== (initial.target.kind === "bundled") ||
            (selected && setupBase(selected) !== host.base) ||
            (!bundled && !selected) ||
            supplied.target.kind !== initial.target.kind ||
            supplied.target.idp !== initial.target.idp ||
            supplied.target.device !== initial.target.device ||
            supplied.target.storage !== initial.target.storage
        ) throw new Error("Selected target changed");
        if (!bundled) {
            const device = getDeviceApiUrl();
            const storage = env.PUBLIC_STORAGE_BASE_URL;
            if ((device ? setupBase(device) : null) !== initial.target.device ||
                (storage ? setupBase(storage) : null) !== initial.target.storage)
                throw new Error("Selected owner targets changed");
        }
    }
    async function capability(): Promise<string> {
        requireTarget();
        const value = code;
        code = "";
        if (observation.target.kind === "bundled" && !usesRemoteApis()) {
            const local = await readNativeSetupCode(host.base);
            if (!local) throw new Error("Admission retired; refresh readiness.");
            return local;
        }
        return value;
    }
    async function action(kind: "join" | "recover" | "cancel") {
        if (busy || ready) return;
        busy = true; error = "";
        try {
            const credential = await capability();
            if (!active) return;
            requireTarget();
            if (kind === "join") {
                const pending = session.join(credential, exact?.node ?? "My Node", exact?.peer ?? peer);
                if (session.intent) onworking?.();
                await pending;
                if (active && session.intent && !session.observationRequired) {
                    requireTarget();
                    await session.recover(credential);
                }
            } else if (kind === "recover") await session.recover(credential);
            else await session.cancel(credential);
        } catch {
            if (active) error = "Unable to authorize this action. Check HTTPS trust, selected target and protected host access. Retained work is unchanged.";
        } finally {
            if (active && session.intent) onworking?.();
            code = ""; busy = false;
        }
        if (active && session.status?.ready) await refresh();
    }
    async function refresh() {
        if (busy) return;
        busy = true; code = "";
        try {
            const next = await observeOnboarding();
            if (!active) return;
            if (next.target.kind !== initial.target.kind || next.target.idp !== initial.target.idp || next.target.device !== initial.target.device || next.target.storage !== initial.target.storage) {
                error = "The selected target changed. Return to connection settings; this Join cannot transfer its retained intent or capability.";
                return;
            }
            error = "";
            observation = next;
            const current = next.hosts.find(value => value.base === host.base);
            if (current && (next.state !== "blocked" || current.status.state === "unfinished")) await session.observe(current.status);
        } catch {
            if (active) { session.observationRequired = true; error = "Unable to verify current readiness. Check the connection before retrying."; }
        } finally { busy = false; }
    }
</script>

<section class="flex flex-col gap-4" aria-label="Join an Installation">
    <h2 class="text-xl font-semibold">{ready ? "Your Node is ready" : working ? "Joining the Installation" : "Join an existing Installation"}</h2>
    {#if ready}
        <p>Current readiness and public login are verified. Setup does not sign you in.</p>
        <a href={resolve("/signin")}>Sign in</a>
    {:else}
        <p>Verify the trusted peer endpoint ID with its administrator. This joins a Node; connecting to remote services alone does not join or provision bundled services.</p>
        {#if exact}<p class="text-sm break-all">Retained Node: {exact.node}; trusted peer: {exact.peer}</p>{/if}
        {#if exact?.issuer}
            <p role="status">Authenticated Installation: {exact.issuer}</p>
            <p>Confirm this Installation is the one you intend to join before its administrator approves the request.</p>
        {:else if working}
            <p>Authenticated target identity is not yet available. Recover the retained request to inspect it; do not approve an unexpected request.</p>
        {/if}
        {#if working}
            {#if !session.observationRequired && session.status?.state !== "unfinished"}<progress aria-label="Setup in progress"></progress>{/if}
            <p role="status">{session.status?.state === "unfinished" ? "Join stopped or was rejected. Recover the exact request and check with the source administrator before an authorized retry." : "Waiting for approval or provisioning. On the source Node, open Devices → Add a device and enable acceptance. After checking the authenticated Installation shown here, approve this exact pending request. No additional owner choices are required."}</p>
            <p>Closing this page does not cancel accepted work. After reload or restart, recover the same request before retrying. Recovery is not automatic authorization to resume.</p>
        {/if}
        {#if observation.state === "blocked"}<p role="alert">{observation.message}</p>{/if}
        <form class="flex flex-col gap-3" onsubmit={event => { event.preventDefault(); if (canJoin && (exact || /^[a-f0-9]{64}$/.test(peer))) void action("join"); }}>
            <label>Trusted peer endpoint ID
                <input bind:value={peer} placeholder={exact?.peer} autocomplete="off" spellcheck="false" pattern={"[a-f0-9]{64}"} minlength="64" maxlength="64" required disabled={busy || working} />
            </label>
            {#if observation.target.kind !== "bundled"}
                <label>Host setup code<input type="password" bind:value={code} autocomplete="off" spellcheck="false" pattern={"[a-f0-9]{64}"} minlength="64" maxlength="64" required disabled={busy} /></label>
                <p>Authorize only the selected host. Re-enter its private setup code for each action; no bundled capability is sent to remote services.</p>
            {/if}
            <button type="submit" disabled={busy || !canJoin || (!exact && !/^[a-f0-9]{64}$/.test(peer)) || (observation.target.kind !== "bundled" && !code)}>{exact ? "Retry exact Join" : "Request approval and join"}</button>
            <button type="button" onclick={() => action("recover")} disabled={busy || (observation.target.kind !== "bundled" && !code)}>Recover retained Join</button>
            {#if exact}
                <button type="button" onclick={() => action("cancel")} disabled={busy || !exact.issuer || session.observationRequired || (observation.target.kind !== "bundled" && !code)}>Cancel exact Join</button>
            {/if}
        </form>
        <button type="button" onclick={refresh} disabled={busy}>Refresh status</button>
        {#if session.message}<p role={session.observationRequired ? "alert" : "status"}>{session.message}</p>{/if}
    {/if}
    {#if error}<p role="alert">{error}</p>{/if}
    <p class="text-sm break-all">Selected setup host: {host.base}</p>
</section>
