<script lang="ts">
    import { invoke, isTauri } from "@tauri-apps/api/core";
    import { onDestroy, onMount } from "svelte";
    import { goto } from "$app/navigation";
    import { resolve } from "$app/paths";
    import { getOidcClient } from "$lib/common/state/oidc.svelte";
    import {
        resetDevice,
        verifyWebSetupCode,
        isUnifiedSetupHost,
        WebSetupCreate,
    } from "$lib/common/state/setupClient.svelte";

    import IndependentSetup from "$lib/common/components/IndependentSetup.svelte";

    import type { SetupHost } from "$lib/common/state/setupApi";

    let { data }: { data: { stage: string; hosts?: SetupHost[] } } = $props();
    type Intent = {
        purpose: "create" | "join";
        issuer: string;
        peer?: string;
        name?: string;
    };
    let purpose = $derived<"create" | "join">(
        data.hosts?.[0]?.status.state === "joining" ? "join" : "create",
    );
    let pinned = $state(false);
    let name = $state("");
    let password = $derived.by(() => {
        data.hosts;
        return "";
    });
    let issuer = $state("");
    let peer = $state("");
    let node = $state("");
    let web = $state.raw<WebSetupCreate | null>(null);
    let observedHosts = $state.raw<SetupHost[] | undefined>();
    let confirmation = $state("");
    let revocationPeer = $state("");
    let revocationStatus = $state("");
    type ResetTarget = {
        member_id: string;
        issuer: string;
        status: "pending" | "acknowledged";
    };
    let revocationTargets = $state.raw<ResetTarget[]>([]);
    let revocationMember = $state("");
    let peerBearer = $state("");
    async function loadRevocationTargets() {
        revocationTargets = await invoke<ResetTarget[]>(
            "pending_reset_revocations",
        );
        revocationMember ||=
            revocationTargets.find((target) => target.status === "pending")
                ?.member_id ?? "";
    }
    let busy = $state(false);
    let loading = $state(true);
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
        peerBearer = "";
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
                        issuer = session.intent.issuer ?? "";
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

    onMount(() => {
        if (!isTauri() || data.hosts) {
            loading = false;
            return;
        }
        void loadRevocationTargets().catch((cause) => {
            error = String(cause);
        });
        invoke<Intent | null>("get_setup_intent")
            .then((intent) => {
                if (intent) {
                    purpose = intent.purpose;
                    issuer = intent.issuer;
                    peer = intent.peer ?? "";
                    name = intent.name ?? "";
                    pinned = true;
                }
            })
            .catch((cause) => {
                error = String(cause);
            })
            .finally(() => {
                loading = false;
            });
    });

    async function provision(event: SubmitEvent) {
        event.preventDefault();
        busy = true;
        error = "";
        try {
            if (purpose === "join") {
                await invoke("join_installation", { peer, name });
            } else {
                await invoke("provision_initial_administrator", {
                    name,
                    password,
                });
            }
            await goto(resolve("/setup/device"));
        } catch (cause) {
            error = cause instanceof Error ? cause.message : String(cause);
            try {
                const intent = await invoke<Intent | null>("get_setup_intent");
                pinned = intent !== null;
                if (intent) {
                    purpose = intent.purpose;
                    issuer = intent.issuer;
                    peer = intent.peer ?? "";
                    if (intent.name) name = intent.name;
                }
            } catch {
                /* Keep the original setup error and allow explicit reset. */
            }
        } finally {
            password = "";
            busy = false;
        }
    }

    async function retryRuntime() {
        busy = true;
        error = "";
        try {
            await invoke("retry_local_runtime");
            await goto(resolve("/setup"));
        } catch (cause) {
            error = String(cause);
        } finally {
            busy = false;
        }
    }

    async function recoverReferences() {
        busy = true;
        error = "";
        try {
            await invoke("recover_reset_references");
            error =
                "Deletion references recovered. Damaged files were archived. This does not restore trust; explicitly retry reset.";
        } catch (cause) {
            error = String(cause);
        } finally {
            busy = false;
        }
    }

    async function revokeOnPeer() {
        const bearer =
            peerBearer ||
            getOidcClient().getStoredTokenResponse()?.access_token;
        if (!bearer) {
            error =
                "Sign in with an authorized User session before revoking on a peer.";
            return;
        }
        if (
            !confirm(
                `Revoke this Node's signer and Device on trusted peer ${revocationPeer}?`,
            )
        )
            return;
        busy = true;
        error = "";
        revocationStatus = "";
        try {
            await invoke("revoke_reset_on_peer", {
                peerApi: revocationPeer,
                bearer,
                memberId: revocationMember,
            });
            await loadRevocationTargets();
            revocationStatus =
                "The surviving peer acknowledged signer and Device revocation. Other peers learn it through ordinary scoped security synchronization. Local data and secrets are still retained.";
        } catch (cause) {
            error = String(cause);
        } finally {
            peerBearer = "";
            busy = false;
        }
    }

    async function reset() {
        busy = true;
        error = "";
        try {
            await resetDevice();
        } catch (cause) {
            error = String(cause);
        } finally {
            busy = false;
        }
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
                            <a class="btn" href={resolve("/signin")}
                                >Continue to sign in</a
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
            <p role="status">
                Native setup uses a separate path; shared API reconciliation is
                outstanding.
            </p>
            <p>
                Every Node runs IdP, Management and Storage with its own private
                signer. Completed Nodes remain usable offline.
            </p>
            {#if data.stage === "unavailable"}
                <p role="status">
                    This Node completed setup, but its local services are
                    unavailable. Its persisted completion and keys have not been
                    discarded. This is not a peer-disconnection authorization
                    failure.
                </p>
                <button onclick={retryRuntime} disabled={busy || loading}
                    >Retry local services</button
                >
            {:else if data.stage === "ready"}
                <p role="status">
                    This Node completed local setup and is ready to use offline.
                    Local completion does not sign you in or grant permissions.
                </p>
                <a class="btn" href={resolve("/signin")}>Sign in</a>
            {:else if data.stage === "device"}
                <p role="status">
                    Installation setup is complete. Continue to Device setup to
                    finish configuring this local Node.
                </p>
                <a class="btn" href={resolve("/setup/device")}
                    >Continue Device setup</a
                >
            {:else if data.stage === "installation"}
                <form onsubmit={provision}>
                    <label>
                        Installation
                        <select
                            bind:value={purpose}
                            disabled={busy || loading || pinned}
                        >
                            <option value="create"
                                >Create an Installation</option
                            >
                            <option value="join"
                                >Join through a trusted peer</option
                            >
                        </select>
                    </label>
                    {#if purpose === "join"}
                        <p>
                            Verify the peer endpoint identity out of band. The
                            app learns the Installation issuer through that
                            authenticated peer. Enable join acceptance and open
                            its authorized approval interface. The trusted peer
                            must explicitly approve this Node using an
                            authorized User session; a password or pairing
                            exchange alone is not approval.
                        </p>

                        <label>
                            Trusted peer endpoint ID
                            <input
                                bind:value={peer}
                                required
                                disabled={busy || loading || pinned}
                            />
                        </label>
                        <label>
                            Local Node name
                            <input
                                bind:value={name}
                                required
                                maxlength="256"
                                disabled={busy || loading || pinned}
                            />
                        </label>
                    {:else}
                        <p>
                            Restricted local setup creates the initial User and
                            exact administrator grants. No password is sent to a
                            peer.
                        </p>
                        <label>
                            Administrator name
                            <input
                                bind:value={name}
                                autocomplete="username"
                                required
                                disabled={busy || loading}
                            />
                        </label>
                        <label>
                            Password
                            <input
                                bind:value={password}
                                type="password"
                                autocomplete="new-password"
                                required
                                disabled={busy || loading}
                            />
                        </label>
                    {/if}
                    {#if pinned}
                        <p role="status">
                            Retained Installation issuer: {issuer}
                        </p>
                    {/if}
                    <button type="submit" disabled={busy || loading}>
                        {busy
                            ? "Preparing Node…"
                            : pinned
                              ? "Retry local setup"
                              : purpose === "join"
                                ? "Request approval and join"
                                : "Create local Node"}
                    </button>
                </form>
                {#if busy && purpose === "join"}
                    <button
                        onclick={() =>
                            invoke("cancel_installation_setup").catch(
                                (cause) => {
                                    error = String(cause);
                                },
                            )}>Cancel pending join</button
                    >
                {/if}
            {/if}
            <p role="status">
                Interrupted setup keeps its identities and remains incomplete.
                An unavailable peer can delay join, but does not invalidate a
                completed Node.
            </p>
            <details>
                <summary>Reset this local Node</summary>
                <p>
                    Reset is explicit and works offline. It removes only this
                    Node's data and local keys, not remote data. Failed secret
                    deletion retains retry state. The app exits after successful
                    reset.
                </p>
                <p>
                    Offline local reset does not revoke membership on surviving
                    peers. When an authorized User session and trusted peer are
                    available, revoke there first or retry after reset using the
                    retained public member reference. This uses no persisted
                    bearer and never treats a local reset as remote
                    acknowledgement.
                </p>
                <label
                    >Revocation target
                    <select bind:value={revocationMember} disabled={busy}>
                        <option value="">Select retained member</option>
                        {#each revocationTargets as target (target.member_id)}
                            <option value={target.member_id}
                                >{target.member_id} — {target.issuer} ({target.status})</option
                            >
                        {/each}
                    </select>
                </label>
                <label
                    >Authorized peer User access token (optional while signed
                    in)
                    <input
                        type="password"
                        bind:value={peerBearer}
                        autocomplete="off"
                        disabled={busy}
                    />
                </label>
                <p>
                    This transient token is sent only to the explicitly selected
                    trusted HTTPS peer after its Installation issuer matches. It
                    is cleared after the attempt and never stored with the reset
                    record.
                </p>
                <label
                    >Trusted surviving peer IdP URL<input
                        type="url"
                        bind:value={revocationPeer}
                        placeholder="https://peer.example/idp"
                        disabled={busy}
                    /></label
                >
                <button
                    onclick={revokeOnPeer}
                    disabled={busy || !revocationPeer || !revocationMember}
                    >Revoke on surviving peer</button
                >
                {#if revocationStatus}<p role="status">
                        {revocationStatus}
                    </p>{/if}
                <label
                    >Type RESET to confirm <input
                        bind:value={confirmation}
                        autocomplete="off"
                        disabled={busy}
                    /></label
                >
                <button
                    onclick={reset}
                    disabled={busy || confirmation !== "RESET"}
                    >Reset / retry pending reset</button
                >
                <button
                    onclick={recoverReferences}
                    disabled={busy || confirmation !== "RESET"}
                    >Recover deletion references from durable backup</button
                >
                <p>
                    Recovery is reset-only: it preserves damaged evidence and
                    does not restore readiness, regenerate a key, or create new
                    trust. Without a valid durable backup, it fails closed.
                </p>
            </details>
        {/if}
        {#if error}<p role="alert">{error}</p>{/if}
    </div>
</div>
