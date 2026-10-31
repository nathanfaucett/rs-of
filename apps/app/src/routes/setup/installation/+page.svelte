<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import { resolve } from "$app/paths";
    import IndependentSetup from "$lib/common/components/IndependentSetup.svelte";
    import {
        observeOnboarding,
        type OnboardingObservation,
    } from "$lib/common/state/onboarding";
    import { Alert, Card, Page, PageHeader } from "$lib/ui";
    let { data }: { data: { observation: OnboardingObservation } } = $props();
    let refreshed = $state.raw<{
        source: typeof data;
        observation: OnboardingObservation;
    }>();
    const observation = $derived(
        refreshed?.source === data ? refreshed.observation : data.observation,
    );
    const bundled = $derived(observation.target.kind === "bundled");
    let busy = $state(false);
    let error = $state("");
    async function refreshObservation() {
        const source = data;
        const next = await observeOnboarding();
        if (source === data) refreshed = { source, observation: next };
    }
    async function runtime(command: string, args?: Record<string, unknown>) {
        if (!bundled || busy) return;
        busy = true;
        error = "";
        try {
            await invoke(command, args);
            await refreshObservation();
        } catch {
            error = "Local services could not start. Retained owner state is unchanged; observe or recover it.";
        } finally {
            busy = false;
        }
    }
    async function retryRuntime() {
        await runtime("retry_local_runtime");
    }
    async function trustConsent(allow: boolean) {
        await runtime("set_local_trust_consent", { consent: allow });
    }
    $effect(() => {
        if (observation.state !== "starting") return;
        const interval = setInterval(() => void refreshObservation(), 500);
        return () => clearInterval(interval);
    });
</script>
<Page width="narrow">
    <PageHeader title="Installation setup" />
    <Card>
        {#if observation.state === "blocked" && observation.hosts.some((host) => host.status.state === "unfinished")}
            <Alert tone="danger">{observation.message}</Alert>
        {/if}
        {#if observation.state === "starting"}
            <Alert tone="info"
                >Starting bundled services. Readiness has not yet been verified.</Alert
            >
            <button
                class="btn btn-secondary btn-md"
                type="button"
                onclick={refreshObservation}>Refresh status</button
            >
        {:else if observation.state === "blocked" && !observation.hosts.some((host) => host.status.state === "unfinished")}
            <Alert tone="danger">{observation.message}</Alert>
            {#if bundled && observation.action === "trust_consent"}
                <p>
                    Allow the operating system to trust this Node's retained
                    localhost certificate authority for HTTPS. This adds a trust
                    anchor, may request administrator elevation, and can affect
                    other applications using the OS trust store. It does not
                    replace your certificate or sign you in. Reset does not
                    remove installed OS trust.
                </p>
                <div class="flex flex-row gap-4">
                    <button
                        class="btn btn-primary btn-md"
                        type="button"
                        onclick={() => trustConsent(true)}
                        disabled={busy}>Allow certificate trust</button
                    >
                    <button
                        class="btn btn-secondary btn-md"
                        type="button"
                        onclick={() => trustConsent(false)}
                        disabled={busy}>Not now</button
                    >
                </div>
            {/if}
            {#if busy}
                <Alert tone="info"
                    >Checking retained host and HTTPS trust. Waiting for
                    platform work to finish.</Alert
                >
            {/if}
            {#if observation.action === "repair"}
                <p>
                    Repair retained evidence before continuing. Reset is not a
                    repair action.
                </p>
                {#if bundled}
                    <a href={resolve("/maintenance")}
                        >Open maintenance for exact-reference recovery</a
                    >
                {/if}
            {/if}
            <div class="flex flex-row gap-4">
                <button
                    class="btn btn-secondary btn-md"
                    type="button"
                    onclick={refreshObservation}
                    disabled={busy}>Retry observation</button
                >
                {#if bundled && observation.action !== "repair"}
                    <button
                        class="btn btn-primary btn-md"
                        type="button"
                        onclick={retryRuntime}
                        disabled={busy}>Retry local services</button
                    >
                {/if}
            </div>
            {#if ["prerequisite", "trust_consent", "trust_declined"].includes(observation.action)}
                <a href={resolve("/config")}>Connection settings</a>
            {/if}
        {:else if observation.state === "ready"}
            <p>Selected services are ready. Setup does not sign you in.</p>
            <p>
                Rename this Node in Devices. Choose local data and Residency
                after signing in; selection starts empty and resource
                permissions still apply.
            </p>
            <a href={resolve("/signin")} class="btn btn-primary btn-md"
                >Sign in</a
            >
        {:else if observation.hosts.length === 3}
            <a href={resolve("/config")}>Advanced connection settings</a>
            {#key observation.hosts}<IndependentSetup hosts={observation.hosts} {bundled} />{/key}
        {:else}
            <Alert tone="danger">Configure all three owner bases before setup.</Alert>
        {/if}
        {#if error}<Alert tone="danger">{error}</Alert>{/if}
    </Card>
</Page>
