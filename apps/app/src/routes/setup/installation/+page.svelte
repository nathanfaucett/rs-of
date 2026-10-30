<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import { onDestroy } from "svelte";
    import { resolve } from "$app/paths";
    import IndependentSetup from "$lib/common/components/IndependentSetup.svelte";

    import JoinSetup from "$lib/common/components/JoinSetup.svelte";
    import {
        type OnboardingObservation,
        observeOnboarding,
    } from "$lib/common/state/onboarding";

    import type { SetupHost } from "$lib/common/state/setupApi";
    import {
        isUnifiedSetupHost,
        readNativeSetupCode,
        usesRemoteApis,
        verifyWebSetupCode,
        WebSetupCreate,
    } from "$lib/common/state/setupClient.svelte";
    import { Alert, Card, Field, Page, PageHeader } from "$lib/ui";

    let { data: loaded }: { data: { observation: OnboardingObservation } } =
        $props();
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
    const bundledCreate = $derived(data.bundled && purpose === "create");
    let joinStarted = $state(false);
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
        if (
            purpose === "join" ||
            !setupStarted ||
            webStatus?.state === "unfinished"
        )
            return;
        const interval = setInterval(() => void observeWeb(), 3000);
        return () => clearInterval(interval);
    });

    $effect(() => {
        if (observation.state !== "starting") return;
        const interval = setInterval(() => void refreshObservation(), 500);
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
        if (!canCreate || busy || !selectedHost) return;
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
                code = (await readNativeSetupCode(selectedHost.base)) ?? "";
                if (!active) return;
            }
            await web.submit(
                code,
                name,
                web.intent?.node ?? "My Node",
                credential,
            );
        } catch {
            error =
                "Protected bundled setup admission is unavailable. Retry without changing the retained identity.";
        } finally {
            setupCode = "";
            password = "";
            busy = false;
        }
        if (active && web?.status?.ready) await observeWeb();
    }

    async function trustConsent(consent: boolean) {
        if (busy || observation.target.kind !== "bundled" || usesRemoteApis())
            return;
        busy = true;
        error = "";
        try {
            await invoke("set_local_trust_consent", { consent });
        } catch {
            error =
                "Trust installation or HTTPS verification failed. Check platform prerequisites, then retry; retained identity is unchanged.";
        } finally {
            busy = false;
        }
        await refreshObservation();
    }

    async function retryRuntime() {
        if (busy || observation.target.kind !== "bundled" || usesRemoteApis())
            return;
        busy = true;
        setupCode = "";
        password = "";
        try {
            await invoke("retry_local_runtime");
        } catch {
            error =
                "Local services could not restart. Check prerequisites; retained identity is unchanged.";
        } finally {
            busy = false;
        }
        await refreshObservation();
    }

    async function refreshObservation() {
        if (busy) return;
        busy = true;
        try {
            const next = await observeOnboarding();
            if (active) observation = next;
        } finally {
            busy = false;
        }
    }

    async function observeWeb() {
        if (busy || !selectedHost) return;
        const base = selectedHost.base;
        if (web && web.base !== base) {
            error =
                "The configured target changed. This session cannot transfer its retained intent to another host.";
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
            const host = snapshot.hosts.find((host) => host.base === base);
            if (snapshot.state !== "blocked" && host)
                await web.observe(host.status);
            observedHosts = snapshot.hosts;
            observation = snapshot;
        } finally {
            busy = false;
        }
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
            if (bundledCreate)
                code = (await readNativeSetupCode(session.base)) ?? "";
            if (!active) return;
            await session.recover(code);
            if (
                bundledCreate &&
                session.intent &&
                !session.observationRequired
            ) {
                session.message =
                    "Exact Create recovered. Re-enter the original administrator password to retry. Closing this page does not cancel accepted work.";
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
                    if ("administrator" in session.intent) {
                        purpose = "create";
                        name = session.intent.administrator;
                    } else {
                        purpose = "join";
                    }
                }
            }
        } catch {
            error =
                "Protected setup admission could not be read. Retained setup is unchanged.";
        } finally {
            setupCode = "";
            password = "";
            busy = false;
        }
        if (active && session.status?.ready) await observeWeb();
    }

    async function cancelWeb() {
        if (busy || !web?.intent || web.base !== selectedHost?.base || !unified)
            return;
        busy = true;
        let code = setupCode;
        setupCode = "";
        password = "";
        try {
            if (bundledCreate)
                code = (await readNativeSetupCode(web.base)) ?? "";
            if (active) await web.cancel(code);
        } catch {
            error =
                "Protected setup admission could not be read. Retained setup is unchanged.";
        } finally {
            busy = false;
        }
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

<Page width="narrow">
    <PageHeader
        title={setupStarted ? progressTitle : "Installation setup"}
        description={setupStarted ? progressDescription : undefined}
    />
    <Card>
        {#if observation.state === "blocked" && data.hosts.some((host) => host.status.state === "unfinished")}
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
        {:else if observation.state === "blocked" && !data.hosts.some((host) => host.status.state === "unfinished")}
            <Alert tone="danger">{observation.message}</Alert>
            {#if data.bundled && observation.action === "trust_consent"}
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
                {#if data.bundled}
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
                {#if data.bundled && observation.action !== "repair"}
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
        {:else if data.hosts?.length === 3}
            {#key data.hosts}<IndependentSetup hosts={data.hosts} />{/key}
        {:else}
            {#if webStatus?.stage === "installation" && !webStatus.ready && !setupStarted}
                <p>Create a new Installation or join an existing one.</p>
                <a href={resolve("/config")}>Advanced connection settings</a>
                <Field label="Installation setup choice">
                    {#snippet children({
                        id,
                        "aria-describedby": describedby,
                        "aria-invalid": invalid,
                        required,
                    })}
                        <select
                            {id}
                            aria-describedby={describedby}
                            aria-invalid={invalid}
                            {required}
                            bind:value={purpose}
                            onchange={() => {
                                setupCode = "";
                                admissionVerified = false;
                                verifiedBase = "";
                                password = "";
                            }}
                            disabled={busy || !!web?.intent || joinStarted}
                        >
                            <option value="create"
                                >Create an Installation</option
                            >
                            <option value="join"
                                >Join through a trusted peer</option
                            >
                        </select>
                    {/snippet}
                </Field>
            {/if}
            {#if purpose === "join" && unified}
                {#key selectedHost?.base}<JoinSetup
                        {observation}
                        onworking={() => {
                            joinStarted = true;
                        }}
                    />{/key}
            {:else if data.hosts?.length}
                {#snippet setupForm()}
                    <form
                        onsubmit={unified ? webSubmit : verifyAdmission}
                        class="flex flex-col gap-4"
                    >
                        {#if webStatus?.stage === "installation" && !webStatus.ready}
                            {#if !bundledCreate}
                                {#if data.bundled}
                                    <button
                                        class="btn btn-secondary btn-md"
                                        type="button"
                                        onclick={loadNativeCode}
                                        disabled={busy}
                                        >Get bundled host code</button
                                    >
                                {/if}
                                <Field
                                    label="Host setup code"
                                    description="Use the code from your server's setup-code file. Keep it private; re-enter it for each action."
                                    required
                                >
                                    {#snippet children({
                                        id,
                                        "aria-describedby": describedby,
                                        "aria-invalid": invalid,
                                        required,
                                    })}
                                        <input
                                            {id}
                                            aria-describedby={describedby}
                                            aria-invalid={invalid}
                                            {required}
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
                                            disabled={busy}
                                        />
                                    {/snippet}
                                </Field>
                            {/if}
                            {#if unified}
                                {#if purpose === "create"}
                                    {#if canCreate || (web?.intent && "administrator" in web.intent)}
                                        <Field
                                            label="Administrator username"
                                            required
                                        >
                                            {#snippet children({
                                                id,
                                                "aria-describedby": describedby,
                                                "aria-invalid": invalid,
                                                required,
                                            })}
                                                <input
                                                    {id}
                                                    aria-describedby={describedby}
                                                    aria-invalid={invalid}
                                                    {required}
                                                    bind:value={name}
                                                    autocomplete="username"
                                                    disabled={busy ||
                                                        !!web?.intent}
                                                />
                                            {/snippet}
                                        </Field>
                                        <Field label="Password" required>
                                            {#snippet children({
                                                id,
                                                "aria-describedby": describedby,
                                                "aria-invalid": invalid,
                                                required,
                                            })}
                                                <input
                                                    {id}
                                                    aria-describedby={describedby}
                                                    aria-invalid={invalid}
                                                    {required}
                                                    type="password"
                                                    bind:value={password}
                                                    autocomplete="new-password"
                                                    disabled={busy}
                                                />
                                            {/snippet}
                                        </Field>
                                        <button
                                            class="btn btn-primary btn-md"
                                            type="submit"
                                            disabled={busy ||
                                                !canCreate ||
                                                (!bundledCreate &&
                                                    !setupCode) ||
                                                !name.trim() ||
                                                !password}
                                            >{web?.intent
                                                ? "Retry exact Create"
                                                : "Create Node"}</button
                                        >
                                    {:else}
                                        <Alert tone="danger">
                                            Setup is already in progress. Enter
                                            your setup code and use Resume setup
                                            below. Retrying Create also requires
                                            the original password.
                                        </Alert>
                                    {/if}
                                {/if}
                                <details open={setupStarted}>
                                    <summary>Recovery and diagnostics</summary>
                                    <div class="flex flex-col gap-3 pt-3">
                                        <p class="text-sm">
                                            Resume after a reload before
                                            retrying or canceling. Closing this
                                            page does not cancel setup.
                                        </p>
                                        <button
                                            class="btn btn-secondary btn-md"
                                            type="button"
                                            onclick={recoverWeb}
                                            disabled={busy ||
                                                (!bundledCreate && !setupCode)}
                                            >Resume setup</button
                                        >
                                        {#if !bundledCreate}
                                            <button
                                                class="btn btn-secondary btn-md"
                                                type="button"
                                                onclick={verifyAdmission}
                                                disabled={busy || !setupCode}
                                                >Verify setup code</button
                                            >
                                        {/if}
                                        {#if web?.intent}
                                            <button
                                                class="btn btn-danger btn-md"
                                                type="button"
                                                onclick={cancelWeb}
                                                disabled={busy ||
                                                    (!bundledCreate &&
                                                        !setupCode) ||
                                                    ("peer" in web.intent &&
                                                        web.observationRequired) ||
                                                    (web.intent &&
                                                        "peer" in web.intent &&
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
                                    class="btn btn-primary btn-md"
                                    type="submit"
                                    disabled={busy ||
                                        !setupCode ||
                                        purpose === "join"}
                                    >Verify setup code</button
                                >
                                <button
                                    class="btn btn-secondary btn-md"
                                    type="button"
                                    disabled
                                    >{purpose === "join"
                                        ? "Request approval and join"
                                        : "Independent Create unavailable"}</button
                                >
                            {/if}
                        {:else if webStatus?.ready && !web?.observationRequired}
                            {#if unified}
                                <Alert tone="info"
                                    >Verifying current selected services and
                                    public login before sign-in.</Alert
                                >
                            {:else}
                                <Alert tone="info">
                                    This owner's readiness does not establish
                                    readiness for the other independent owners.
                                </Alert>
                            {/if}
                        {:else}
                            <Alert tone="danger">
                                Setup status is unavailable. Check the server
                                before retrying.
                            </Alert>
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
                <button
                    class="btn btn-secondary btn-md"
                    type="button"
                    onclick={observeWeb}
                    disabled={busy}
                    >{busy ? "Checking…" : "Refresh status"}</button
                >
                {#if web?.message && !setupStarted && !webStatus?.ready}
                    <Alert tone={web.observationRequired ? "danger" : "info"}
                        >{web.message}</Alert
                    >
                {/if}
                {#if admissionVerified && verifiedBase === selectedHost?.base}
                    <Alert tone="success">
                        Code verified. Re-enter it to continue setup.
                    </Alert>
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
                <Alert tone="danger">
                    Setup host scope is unavailable. Readiness has not been
                    established; no code can be submitted.
                </Alert>
            {/if}
        {/if}
        {#if data.bundled}
            <a href={resolve("/maintenance")}>Local Node maintenance</a>
        {/if}
        {#if error}
            <Alert tone="danger">{error}</Alert>
        {/if}
    </Card>
</Page>
