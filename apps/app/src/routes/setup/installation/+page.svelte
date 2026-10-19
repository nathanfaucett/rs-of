<script lang="ts">
    import { isTauri } from "@tauri-apps/api/core";
    import { onDestroy } from "svelte";
    import { resolve } from "$app/paths";
    import {
        readNativeSetupCode,
        verifyWebSetupCode,
        isUnifiedSetupHost,
        WebSetupCreate,
    } from "$lib/common/state/setupClient.svelte";

    import NativeSetupRecovery from "$lib/common/components/NativeSetupRecovery.svelte";
    import IndependentSetup from "$lib/common/components/IndependentSetup.svelte";

    import type { SetupHost } from "$lib/common/state/setupApi";

    let { data }: { data: { stage: string; hosts?: SetupHost[]; bundled?: boolean; nativeError?: string } } = $props();
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
    let selectedBase = $derived(data.hosts?.[0]?.base ?? "");
    let verifiedBase = $derived.by(() => {
        data.hosts;
        return "";
    });
    let active = true;
    const selectedHost = $derived(
        selectedBase
            ? data.hosts?.find((host) => host.base === selectedBase)
            : data.hosts?.[0],
    );

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

    function selectHost(event: Event) {
        if (web?.intent || busy) return;
        web?.destroy();
        web = null;
        password = "";
        selectedBase = (event.currentTarget as HTMLSelectElement).value;
        setupCode = "";
        admissionVerified = false;
        verifiedBase = "";
        error = "";
    }

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
        busy = true;
        const code = setupCode;
        const credential = password;
        setupCode = "";
        password = "";
        try {
            if (purpose === "join") await web.join(code, node, peer);
            else await web.submit(code, name, node, credential);
        } finally {
            setupCode = "";
            password = "";
            busy = false;
        }
    }

    async function observeWeb() {
        if (busy || !selectedHost) return;
        if (web && web.base !== selectedHost.base) {
            error =
                "The configured target changed. This session cannot transfer its retained intent to another host.";
            setupCode = "";
            password = "";
            return;
        }
        web ??= new WebSetupCreate(selectedHost);
        const hosts = data.hosts;
        busy = true;
        await web.observe();
        if (!web.observationRequired) observedHosts = hosts;
        if (web.status?.ready) {
            setupCode = "";
            password = "";
        }
        busy = false;
    }

    async function recoverWeb() {
        if (busy || !unified || !selectedHost) return;
        const code = setupCode;
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
            await session.recover(code);
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
        } finally {
            setupCode = "";
            password = "";
            busy = false;
        }
    }

    async function cancelWeb() {
        if (busy || !web?.intent || web.base !== selectedHost?.base || !unified)
            return;
        busy = true;
        const code = setupCode;
        setupCode = "";
        password = "";
        await web.cancel(code);
        busy = false;
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
            if (code === null) error = "Bundled setup admission is retired; observe readiness.";
            else setupCode = code;
        } catch { error = "Protected bundled code retrieval failed or the selected target changed. No code was sent."; }
        finally { busy = false; }
    }
</script>

<div class="flex grow flex-col items-center justify-center">
    <div class="card w-sm">
        <h1>Installation setup</h1>
        {#if data.hosts?.length === 3}
            {#key data.hosts}<IndependentSetup hosts={data.hosts} />{/key}
        {:else if !isTauri() || data.hosts}
            <p>
                Retrieve each host-generated setup code through your protected
                deployment-management channel. Do not put it in a URL or logs.
                Your deployment must install the required OS CA trust; this
                browser cannot perform privileged OS installation. The current
                HTTPS UI origin must be explicitly allowed by each backend.
            </p>
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
                    <option value="create">Create an Installation</option>
                    <option value="join">Join through a trusted peer</option>
                </select>
            </label>
            <p role="status">
                Unified API Create and Join controls are available.
                Code verification alone cannot complete setup; observe verified readiness.
            </p>
            <p>
                Credentials are transient. After a browser reload, recover the
                exact intent using the host code before retrying or canceling
                retained Create or Join. Public status alone does not reveal
                that intent. Recovery does not generate a nonce, retry or cancel
                work. Closing this page does not cancel host-owned work. Exact
                Create retries require the original password. Join retries
                retain the exact Node and pinned peer; cancellation requires the
                recovered issuer too. Re-enter the host code for every
                privileged request.
            </p>
            <p>
                On the source Node, sign in as an authorized User and open
                <a href={resolve("/devices")}>Devices → Equal-Node joining</a>
                to enable acceptance, inspect the exact offer and approve or reject
                it. Use the source app configured for that Node, not this joining
                host. Setup codes never authorize source approval; approval alone
                is not readiness.
            </p>
            {#if data.hosts?.length}
                <p>
                    Owner scope describes only this host's setup-code authority,
                    not membership, domain permissions or readiness. Independent
                    hosts require their own codes; a unified host uses one code
                    at the displayed IdP API base only.
                </p>
                {#each data.hosts as host (host.base)}
                    <p>
                        <strong>{host.status.owners.join(", ")}</strong>:
                        {host.base} — {host.base === selectedHost?.base
                            ? webStatus?.state
                            : host.status.state}
                    </p>
                {/each}
                <form onsubmit={unified ? webSubmit : verifyAdmission}>
                    <label>
                        Setup host / exact API base
                        <select
                            value={selectedHost?.base ?? ""}
                            onchange={selectHost}
                            disabled={busy || !!web?.intent}
                        >
                            {#each data.hosts as host (host.base)}
                                <option value={host.base}>
                                    {host.status.owners.join(", ")} — {host.base}
                                </option>
                            {/each}
                        </select>
                    </label>
                    {#if webStatus?.stage === "installation" && !webStatus.ready}
                        {#if data.bundled}<button type="button" onclick={loadNativeCode} disabled={busy}>Get bundled host code</button>{/if}
                        <label>
                            Host setup code
                            <input
                                type="password"
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
                        {#if unified}
                            {#if purpose === "create"}
                                {#if canCreate || (web?.intent && "administrator" in web.intent)}
                                    <label>
                                        Administrator name
                                        <input
                                            bind:value={name}
                                            autocomplete="username"
                                            required
                                            disabled={busy || !!web?.intent}
                                        />
                                    </label>
                                    <label>
                                        Node name
                                        <input
                                            bind:value={node}
                                            autocomplete="off"
                                            required
                                            disabled={busy || !!web?.intent}
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
                                            !setupCode ||
                                            !name.trim() ||
                                            !node.trim() ||
                                            !password}
                                    >
                                        {web?.intent
                                            ? "Retry exact Create"
                                            : "Create Node"}
                                    </button>
                                {:else}
                                    <p role="alert">
                                        Recover the exact retained intent before
                                        retrying or canceling. Enter the host
                                        code; recovery restores the locked
                                        administrator and Node names, not the
                                        password. Observation alone does not
                                        recover intent.
                                    </p>
                                {/if}
                            {:else}
                                <p>
                                    Verify the canonical peer endpoint identity
                                    out of band. The host authenticates the
                                    pinned peer and discovers the issuer; do not
                                    enter an issuer or source credentials here.
                                </p>
                                {#if canJoin || (web?.intent && "peer" in web.intent)}
                                    <label>
                                        Node name
                                        <input
                                            bind:value={node}
                                            autocomplete="off"
                                            required
                                            disabled={busy || !!web?.intent}
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
                                            disabled={busy || !!web?.intent}
                                        />
                                    </label>
                                    {#if web?.intent && "peer" in web.intent && web.intent.issuer}
                                        <p>
                                            Recovered Installation issuer: {web
                                                .intent.issuer}
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
                                        Recover the exact retained intent before
                                        retrying or canceling. Observation alone
                                        does not recover intent.
                                    </p>
                                {/if}
                            {/if}
                            <button
                                type="button"
                                onclick={recoverWeb}
                                disabled={busy || !setupCode}
                                >Recover retained Create or Join</button
                            >
                            <button
                                type="button"
                                onclick={verifyAdmission}
                                disabled={busy || !setupCode}
                                >Verify setup code</button
                            >
                            {#if web?.intent}
                                <button
                                    type="button"
                                    onclick={cancelWeb}
                                    disabled={busy ||
                                        !setupCode ||
                                        ("peer" in web.intent &&
                                            web.observationRequired) ||
                                        (web.intent &&
                                            "peer" in web.intent &&
                                            !web.intent.issuer)}
                                    >Cancel exact {"peer" in web.intent
                                        ? "Join"
                                        : "Create"}</button
                                >
                            {/if}
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
                            <p role="status">
                                Node readiness verified. Setup does not sign you
                                in or grant resource access.
                            </p>
                            <a class="btn" href={resolve(data.bundled ? "/setup/device" : "/signin")}
                                >{data.bundled ? "Continue Device setup" : "Continue to sign in"}</a
                            >
                        {:else}
                            <p role="status">
                                This owner's readiness does not establish
                                readiness for the other independent owners.
                            </p>
                        {/if}
                    {:else}
                        <p role="alert">
                            This host's setup state is unavailable. Restore its
                            protected host state before retrying; readiness has
                            not been established.
                        </p>
                    {/if}
                </form>
                <button type="button" onclick={observeWeb} disabled={busy}
                    >Observe host status</button
                >
                {#if web?.message}<p
                        role={web.observationRequired ? "alert" : "status"}
                    >
                        {web.message}
                    </p>{/if}
                {#if admissionVerified && verifiedBase === selectedHost?.base}
                    <p role="status">
                        Setup code accepted for {verifiedBase} only. This does not
                        complete Installation Setup, approve membership, grant permissions
                        or sign you in. The code has been cleared; future mutations
                        must carry their own admission.
                    </p>
                {/if}
            {:else}
                <p role="alert">
                    Setup host scope is unavailable. Readiness has not been
                    established; no code can be submitted.
                </p>
            {/if}
        {:else}
            <p role="alert">Shared HTTPS setup is unavailable. Restore the selected host prerequisites; no replacement identity is created.</p>
        {/if}
        {#if data.bundled}<NativeSetupRecovery stage={data.stage} nativeError={data.nativeError} />{/if}
        {#if error}<p role="alert">{error}</p>{/if}
    </div>
</div>
