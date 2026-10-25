<script lang="ts">
    import { onDestroy } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import { resolve } from "$app/paths";
    import {
        readNativeSetupCode,
        verifyWebSetupCode,
        isUnifiedSetupHost,
        WebSetupCreate,
        usesRemoteApis,
    } from "$lib/common/state/setupClient.svelte";

    import NativeSetupRecovery from "$lib/common/components/NativeSetupRecovery.svelte";
    import IndependentSetup from "$lib/common/components/IndependentSetup.svelte";

    import type { SetupHost } from "$lib/common/state/setupApi";

    import { observeOnboarding, type OnboardingObservation } from "$lib/common/state/onboarding";

    let { data: loaded }: { data: { observation: OnboardingObservation } } = $props();
    let observation = $derived(loaded.observation);
    const data = $derived({
        hosts: observation.hosts,
        bundled: observation.target.kind === "bundled",
    });
    let purpose = $derived<"create" | "join">(
        data.hosts?.[0]?.status.state === "joining" ? "join" : "create",
    );
    let name = $state("");
    let password = $derived.by(() => {
        data.hosts;
        return "";
    });
    let peer = $state("");
    let node = $state("");
    const bundledCreate = $derived(data.bundled && purpose === "create");
    let web = $state.raw<WebSetupCreate | null>(null);
    let observedHosts = $state.raw<SetupHost[] | undefined>();
    let busy = $state(false);
    let error = $state("");
    let setupCode = $derived.by(() => {
        data.hosts;
        return "";
    });
    let admissionVerified = $derived.by(() => {
        data.hosts;
        return false;
    });

    let verifiedBase = $derived.by(() => {
        data.hosts;
        return "";
    });
    let active = true;
    const selectedHost = $derived(data.hosts?.[0]);

    const webStatus = $derived(
        web?.base === selectedHost?.base && observedHosts === data.hosts
            ? web?.status
            : selectedHost?.status,
    );
    const unified = $derived(
        data.hosts?.length === 1 &&
            selectedHost &&
            isUnifiedSetupHost(selectedHost),
    );
    const canCreate = $derived(
        unified &&
            (!web ||
                (web.base === selectedHost?.base &&
                    observedHosts === data.hosts)) &&
            purpose === "create" &&
            webStatus?.stage === "installation" &&
            !webStatus.ready &&
            !web?.observationRequired &&
            ((web?.intent && "administrator" in web.intent) ||
                (!web?.intent && webStatus.state === "awaiting_choice")),
    );

    const canJoin = $derived(
        unified &&
            (!web ||
                (web.base === selectedHost?.base &&
                    observedHosts === data.hosts)) &&
            purpose === "join" &&
            webStatus?.stage === "installation" &&
            !webStatus.ready &&
            !web?.observationRequired &&
            ((web?.intent && "peer" in web.intent) ||
                (!web?.intent && webStatus.state === "awaiting_choice")),
    );

    const setupStarted = $derived(
        unified &&
            !webStatus?.ready &&
            (!!web?.intent ||
                ["creating", "joining", "unfinished"].includes(
                    webStatus?.state ?? "",
                )),
    );
    const progressTitle = $derived(
        web?.observationRequired
            ? "Unable to confirm setup status"
            : webStatus?.state === "unfinished"
              ? "Setup needs attention"
              : webStatus?.state === "joining" || purpose === "join"
                ? "Joining the Installation"
                : "Creating your Node",
    );
    const progressDescription = $derived(
        web?.observationRequired
            ? "Your server may still be working. Check its connection before retrying."
            : webStatus?.state === "unfinished"
              ? "Setup did not finish. Open recovery options to resume or cancel."
              : webStatus?.state === "joining" || purpose === "join"
                ? "If approval is needed, approve this Node on the source Node. Status updates automatically."
                : "Your server is setting up the Installation. Status updates automatically.",
    );

    $effect(() => {
        if (!setupStarted || webStatus?.state === "unfinished") return;
        const interval = setInterval(() => void observeWeb(), 3000);
        return () => clearInterval(interval);
    });

    onDestroy(() => {
        active = false;
        setupCode = "";
        password = "";
        web?.destroy();
        admissionVerified = false;
        verifiedBase = "";
    });

    async function webSubmit(event: SubmitEvent) {
        event.preventDefault();
        if (
            !(purpose === "create" ? canCreate : canJoin) ||
            busy ||
            !selectedHost
        )
            return;
        web ??= new WebSetupCreate(selectedHost);
        observedHosts = data.hosts;
        error = "";
        busy = true;
        let code = setupCode;
        const credential = password;
        setupCode = "";
        password = "";
        try {
            if (bundledCreate) {
                code = await readNativeSetupCode(selectedHost.base) ?? "";
                if (!active) return;
            }
            if (purpose === "join") await web.join(code, node, peer);
            else await web.submit(code, name, web.intent?.node ?? "My Node", credential);
        } catch {
            error = "Protected bundled setup admission is unavailable. Retry without changing the retained identity.";
        } finally {
            setupCode = "";
            password = "";
            busy = false;
        }
    }

    async function retryRuntime() {
        if (busy || observation.target.kind !== "bundled" || usesRemoteApis()) return;
        busy = true;
        setupCode = "";
        password = "";
        try { await invoke("retry_local_runtime"); }
        catch { error = "Local services could not restart. Check prerequisites; retained identity is unchanged."; }
        finally { busy = false; }
        await refreshObservation();
    }

    async function refreshObservation() {
        if (busy) return;
        busy = true;
        try { observation = await observeOnboarding(); }
        finally { busy = false; }
    }

    async function observeWeb() {
        if (busy || !selectedHost) return;
        const base = selectedHost.base;
        if (web && web.base !== base) {
            error = "The configured target changed. This session cannot transfer its retained intent to another host.";
            setupCode = "";
            password = "";
            return;
        }
        web ??= new WebSetupCreate(selectedHost);
        busy = true;
        try {
            const snapshot = await observeOnboarding();
            if (!active) return;
            setupCode = "";
            password = "";
            const host = snapshot.hosts.find(host => host.base === base);
            if (snapshot.state !== "blocked" && host) await web.observe(host.status);
            observedHosts = snapshot.hosts;
            observation = snapshot;
        } finally { busy = false; }
    }

    async function recoverWeb() {
        if (busy || !unified || !selectedHost) return;
        let code = setupCode;
        setupCode = "";
        password = "";
        admissionVerified = false;
        verifiedBase = "";
        error = "";
        if (web && web.base !== selectedHost.base) {
            error =
                "The configured target changed. This session cannot transfer its retained intent to another host.";
            return;
        }
        web ??= new WebSetupCreate(selectedHost);
        const session = web;
        const hosts = data.hosts;
        busy = true;
        try {
            if (bundledCreate) code = await readNativeSetupCode(session.base) ?? "";
            if (!active) return;
            await session.recover(code);
            if (bundledCreate && session.intent && !session.observationRequired) {
                session.message = "Exact Create recovered. Re-enter the original administrator password to retry. Closing this page does not cancel accepted work.";
            }
            if (
                !active ||
                web !== session ||
                selectedHost?.base !== session.base
            )
                return;
            if (!session.observationRequired) {
                observedHosts = hosts;
                if (session.intent) {
                    node = session.intent.node;
                    if ("administrator" in session.intent) {
                        purpose = "create";
                        name = session.intent.administrator;
                    } else {
                        purpose = "join";
                        peer = session.intent.peer;
                    }
                }
            }
        } catch {
            error = "Protected setup admission could not be read. Retained setup is unchanged.";
        } finally {
            setupCode = "";
            password = "";
            busy = false;
        }
    }

    async function cancelWeb() {
        if (busy || !web?.intent || web.base !== selectedHost?.base || !unified) return;
        busy = true;
        let code = setupCode;
        setupCode = "";
        password = "";
        try {
            if (bundledCreate) code = await readNativeSetupCode(web.base) ?? "";
            if (active) await web.cancel(code);
        } catch {
            error = "Protected setup admission could not be read. Retained setup is unchanged.";
        } finally { busy = false; }
    }

    async function verifyAdmission(event: Event) {
        event.preventDefault();
        busy = true;
        error = "";
        admissionVerified = false;
        const base = selectedHost?.base;
        const code = setupCode;
        setupCode = "";
        verifiedBase = "";
        try {
            if (!base) throw new Error("Setup admission target is unavailable");
            await verifyWebSetupCode(base, code);
            if (active) {
                admissionVerified = true;
                verifiedBase = base;
            }
        } catch {
            if (active) {
                error =
                    "Setup admission failed. Verify the host code, HTTPS trust and configuration.";
            }
        } finally {
            setupCode = "";
            busy = false;
        }
    }

    async function loadNativeCode() {
        if (!data.bundled || busy || !selectedHost) return;
        setupCode = "";
        error = "";
        busy = true;
        const base = selectedHost.base;
        try {
            const code = await readNativeSetupCode(base);
            if (!active || selectedHost?.base !== base) return;
            if (code === null)
                error =
                    "Bundled setup admission is retired; observe readiness.";
            else setupCode = code;
        } catch {
            error =
                "Protected bundled code retrieval failed or the selected target changed. No code was sent.";
        } finally {
            busy = false;
        }
    }
</script>

<div class="flex grow flex-col items-center w-full overflow-auto">
    <div class="flex flex-col w-full max-w-lg gap-4 px-4 py-8">
        <div class="card flex flex-col gap-4">
            <h1 class="text-2xl font-semibold">
                {setupStarted ? progressTitle : "Installation setup"}
            </h1>
            {#if observation.state === "blocked" && data.hosts.some(host => host.status.state === "unfinished")}<p role="alert">{observation.message}</p>{/if}
            {#if observation.state === "starting"}
                <p role="status">Starting bundled services. Readiness has not yet been verified.</p>
                <button onclick={refreshObservation}>Refresh status</button>
            {:else if observation.state === "blocked" && !data.hosts.some(host => host.status.state === "unfinished")}
                <p role="alert">{observation.message}</p>
                {#if observation.action === "repair"}<p>Repair retained evidence before continuing. Reset is not a repair action.</p>{/if}
                <button onclick={refreshObservation} disabled={busy}>Retry observation</button>
                {#if data.bundled && observation.action !== "repair"}
                    <button onclick={retryRuntime} disabled={busy}>Retry local services</button>
                {/if}
                {#if observation.action === "prerequisite"}<a href={resolve("/config")}>Connection settings</a>{/if}
            {:else if data.hosts?.length === 3}
                {#key data.hosts}<IndependentSetup hosts={data.hosts} />{/key}
            {:else if observation.state === "ready"}
                <p>Selected services are ready. Setup does not sign you in.</p>
                <p>Rename this Node in Devices. Choose local data and Residency after signing in; selection starts empty and resource permissions still apply.</p>
                <a href={resolve("/signin")}>Sign in</a>
            {:else}
                {#if webStatus?.stage === "installation" && !webStatus.ready && !setupStarted}
                    <p>Create a new Installation or join an existing one.</p>
                    <a href={resolve("/config")}>Advanced connection settings</a>
                    <label>
                        Installation setup choice
                        <select
                            bind:value={purpose}
                            onchange={() => {
                                setupCode = "";
                                admissionVerified = false;
                                verifiedBase = "";
                                password = "";
                            }}
                            disabled={busy || !!web?.intent}
                        >
                            <option value="create"
                                >Create an Installation</option
                            >
                            <option value="join"
                                >Join through a trusted peer</option
                            >
                        </select>
                    </label>
                {/if}
                {#if data.hosts?.length}
                    {#snippet setupForm()}
                        <form
                            onsubmit={unified ? webSubmit : verifyAdmission}
                            class="flex flex-col gap-4"
                        >
                            {#if webStatus?.stage === "installation" && !webStatus.ready}
                                {#if !bundledCreate}
                                {#if data.bundled}<button
                                        type="button"
                                        onclick={loadNativeCode}
                                        disabled={busy}
                                        >Get bundled host code</button
                                    >{/if}
                                <label>
                                    Host setup code
                                    <input
                                        type="password"
                                        aria-describedby="setup-code-hint"
                                        bind:value={setupCode}
                                        oninput={() => {
                                            admissionVerified = false;
                                            verifiedBase = "";
                                        }}
                                        autocomplete="off"
                                        spellcheck="false"
                                        pattern={"[a-f0-9]{64}"}
                                        minlength="64"
                                        maxlength="64"
                                        required
                                        disabled={busy}
                                    />
                                </label>
                                <p
                                    id="setup-code-hint"
                                    class="text-sm opacity-70"
                                >
                                    Use the code from your server's setup-code
                                    file. Keep it private; re-enter it for each
                                    action.
                                </p>
                                {/if}
                                {#if unified}
                                    {#if purpose === "create"}
                                        {#if canCreate || (web?.intent && "administrator" in web.intent)}
                                            <label>
                                                Administrator name
                                                <input
                                                    bind:value={name}
                                                    autocomplete="username"
                                                    required
                                                    disabled={busy ||
                                                        !!web?.intent}
                                                />
                                            </label>
                                            <label>
                                                Password
                                                <input
                                                    type="password"
                                                    bind:value={password}
                                                    autocomplete="new-password"
                                                    required
                                                    disabled={busy}
                                                />
                                            </label>
                                            <button
                                                type="submit"
                                                disabled={busy ||
                                                    !canCreate ||
                                                    (!bundledCreate && !setupCode) ||
                                                    !name.trim() ||
                                                    !password}
                                            >
                                                {web?.intent
                                                    ? "Retry exact Create"
                                                    : "Create Node"}
                                            </button>
                                        {:else}
                                            <p role="alert">
                                                Setup is already in progress.
                                                Enter your setup code and use
                                                Resume setup below. Retrying
                                                Create also requires the
                                                original password.
                                            </p>
                                        {/if}
                                    {:else}
                                        <p>
                                            Verify the peer's endpoint ID with
                                            its administrator. On the source
                                            Node, enable and approve joining in
                                            Devices → Equal-Node joining.
                                        </p>
                                        {#if canJoin || (web?.intent && "peer" in web.intent)}
                                            <label>
                                                Node name
                                                <input
                                                    bind:value={node}
                                                    autocomplete="off"
                                                    required
                                                    disabled={busy ||
                                                        !!web?.intent}
                                                />
                                            </label>
                                            <label>
                                                Trusted peer endpoint ID
                                                <input
                                                    bind:value={peer}
                                                    autocomplete="off"
                                                    spellcheck="false"
                                                    pattern="[a-f0-9]{64}"
                                                    required
                                                    disabled={busy ||
                                                        !!web?.intent}
                                                />
                                            </label>
                                            {#if web?.intent && "peer" in web.intent && web.intent.issuer}
                                                <p>
                                                    Recovered Installation
                                                    issuer: {web.intent.issuer}
                                                </p>
                                            {/if}
                                            <button
                                                type="submit"
                                                disabled={busy ||
                                                    !canJoin ||
                                                    !setupCode ||
                                                    !node ||
                                                    !peer}
                                            >
                                                {web?.intent
                                                    ? "Retry exact Join"
                                                    : "Request approval and join"}
                                            </button>
                                        {:else}
                                            <p role="alert">
                                                Setup is already in progress.
                                                Enter your setup code and use
                                                Resume setup below.
                                            </p>
                                        {/if}
                                    {/if}
                                    <details open={setupStarted}>
                                        <summary
                                            >Recovery and diagnostics</summary
                                        >
                                        <div class="flex flex-col gap-3 pt-3">
                                            <p class="text-sm">
                                                Resume after a reload before
                                                retrying or canceling. Closing
                                                this page does not cancel setup.
                                            </p>
                                            <button
                                                type="button"
                                                onclick={recoverWeb}
                                                disabled={busy || (!bundledCreate && !setupCode)}
                                                >Resume setup</button
                                            >
                                            {#if !bundledCreate}<button
                                                type="button"
                                                onclick={verifyAdmission}
                                                disabled={busy || !setupCode}
                                                >Verify setup code</button
                                            >{/if}
                                            {#if web?.intent}
                                                <button
                                                    type="button"
                                                    onclick={cancelWeb}
                                                    disabled={busy ||
                                                        (!bundledCreate && !setupCode) ||
                                                        ("peer" in web.intent &&
                                                            web.observationRequired) ||
                                                        (web.intent &&
                                                            "peer" in
                                                                web.intent &&
                                                            !web.intent.issuer)}
                                                    >Cancel exact {"peer" in
                                                    web.intent
                                                        ? "Join"
                                                        : "Create"}</button
                                                >
                                            {/if}
                                        </div>
                                    </details>
                                {:else}
                                    <button
                                        type="submit"
                                        disabled={busy ||
                                            !setupCode ||
                                            purpose === "join"}
                                        >Verify setup code</button
                                    >
                                    <button type="button" disabled
                                        >{purpose === "join"
                                            ? "Request approval and join"
                                            : "Independent Create unavailable"}</button
                                    >
                                {/if}
                            {:else if webStatus?.ready && !web?.observationRequired}
                                {#if unified}
                                    <p role="status">Your Node is ready.</p>
                                    <a
                                        class="btn"
                                        href={resolve("/signin")}
                                        >Continue to sign in</a
                                    >
                                {:else}
                                    <p role="status">
                                        This owner's readiness does not
                                        establish readiness for the other
                                        independent owners.
                                    </p>
                                {/if}
                            {:else}
                                <p role="alert">
                                    Setup status is unavailable. Check the
                                    server before retrying.
                                </p>
                            {/if}
                        </form>
                    {/snippet}
                    {#if setupStarted}
                        <div role="status" class="flex flex-col gap-3">
                            {#if !web?.observationRequired && webStatus?.state !== "unfinished"}
                                <progress
                                    class="w-full"
                                    aria-label="Setup in progress"
                                ></progress>
                            {/if}
                            <p>{progressDescription}</p>
                        </div>
                        <details>
                            <summary>Recovery options</summary>
                            <div class="flex flex-col gap-4 pt-4">
                                {@render setupForm()}
                            </div>
                        </details>
                    {:else}
                        {@render setupForm()}
                    {/if}
                    <button type="button" onclick={observeWeb} disabled={busy}
                        >{busy ? "Checking…" : "Refresh status"}</button
                    >
                    {#if web?.message && !setupStarted && !webStatus?.ready}
                        <p role={web.observationRequired ? "alert" : "status"}>
                            {web.message}
                        </p>
                    {/if}
                    {#if admissionVerified && verifiedBase === selectedHost?.base}
                        <p role="status">
                            Code verified. Re-enter it to continue setup.
                        </p>
                    {/if}
                    <details>
                        <summary>Connection details</summary>
                        <p class="text-sm break-all">{selectedHost?.base}</p>
                        <p class="text-sm">
                            Services: {selectedHost?.status.owners.join(", ")}
                        </p>
                        <p class="text-sm">Status: {webStatus?.state}</p>
                        {#if setupStarted && web?.message}
                            <p class="text-sm break-words">{web.message}</p>
                        {/if}
                    </details>
                {:else}
                    <p role="alert">
                        Setup host scope is unavailable. Readiness has not been
                        established; no code can be submitted.
                    </p>
                {/if}
            {/if}
            {#if data.bundled}<details><summary>Maintenance</summary><NativeSetupRecovery /></details>{/if}
            {#if error}<p role="alert">{error}</p>{/if}
        </div>
    </div>
</div>
