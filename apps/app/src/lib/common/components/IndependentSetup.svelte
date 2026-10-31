<script lang="ts">
    import { onDestroy, untrack } from "svelte";
    import { resolve } from "$app/paths";
    import { Alert, Field } from "$lib/ui";
    import { IndependentSetupSession } from "../state/independentSetup.svelte";
    import {
        independentOwners,
        type OwnerCodes,
    } from "../state/independentSetupApi";
    import { readNativeSetupCode } from "../state/setupClient.svelte";
    import type { SetupHost } from "../state/setupApi";

    let { hosts, bundled = false }: {
        hosts: SetupHost[];
        bundled?: boolean;
    } = $props();
    let active = true;
    let retrieving = $state(false);
    const session = untrack(() => new IndependentSetupSession(hosts));
    let purpose = $state<"create" | "join">(
        untrack(() =>
            hosts.some((host) => host.status.state === "joining")
                ? "join"
                : "create",
        ),
    );
    let administrator = $state("");
    let node = $state("");
    let peer = $state("");
    let password = $state("");
    let codes = $state<OwnerCodes>({ idp: "", device: "", storage: "" });
    const canSubmit = $derived(
        !session.busy &&
            !retrieving &&
            !session.ready &&
            !session.observationRequired &&
            (!!session.intent ||
                session.hosts.every(
                    (host) => host.status.state === "awaiting_choice",
                )),
    );
    function clearSecrets() {
        password = "";
        codes = { idp: "", device: "", storage: "" };
    }
    async function currentCodes(current: OwnerCodes): Promise<OwnerCodes | null> {
        if (!bundled) return current;
        retrieving = true;
        try {
            for (const owner of independentOwners) {
                if (!active) return null;
                if (session.hosts.find(
                    (host) => host.status.owners[0] === owner,
                )?.status.ready) continue;
                const code = await readNativeSetupCode(
                    session.bases[owner], owner,
                );
                if (!code)
                    throw new Error("Owner capability retired; observe readiness");
                current[owner] = code;
            }
            return active ? current : null;
        } catch {
            clearSecrets();
            session.message =
                "Protected owner code retrieval failed or selection changed. Observe the selected owners before retrying.";
            return null;
        } finally {
            retrieving = false;
        }
    }
    async function submit(event: SubmitEvent) {
        event.preventDefault();
        if (retrieving || session.busy) return;
        const supplied = { ...codes };
        const credential = password;
        clearSecrets();
        const current = await currentCodes(supplied);
        if (!current) return;
        await session.submit(
            current,
            {
                purpose,
                administrator,
                node:
                    purpose === "create"
                        ? (session.intent?.node ?? "My Node")
                        : node,
                peer,
            },
            credential,
        );
    }
    async function recover() {
        if (retrieving || session.busy) return;
        const supplied = { ...codes };
        clearSecrets();
        const current = await currentCodes(supplied);
        if (!current) return;
        await session.recover(current);
        if (session.intent) {
            purpose = session.intent.purpose;
            administrator = session.intent.administrator;
            node = session.intent.node;
            peer = session.intent.peer ?? "";
        }
    }
    async function cancel() {
        if (retrieving || session.busy) return;
        const supplied = { ...codes };
        clearSecrets();
        const current = await currentCodes(supplied);
        if (!current) return;
        await session.cancel(current);
    }
    async function observe() {
        if (retrieving || session.busy) return;
        clearSecrets();
        await session.observe();
    }
    onDestroy(() => {
        active = false;
        clearSecrets();
        session.destroy();
    });
</script>

<section aria-labelledby="independent-setup-title" class="flex flex-col gap-3">
    <h2 id="independent-setup-title">Owner Create / Join</h2>
    <p>
        Each owner requires its own code in its own HTTPS Authorization header.
        Codes and passwords are transient, never saved. Bundled setup uses one local
        certificate authority for all owners. Remote deployments require verified HTTPS trust; never bypass certificate warnings.
    </p>
    <p>
        One operation targets the exact three bases below. Accepted work
        continues after this page closes. Partial success is not completion or
        rollback. After reload, recover all current owner tuples before retrying
        or canceling; do not invent a replacement nonce.
    </p>
    <p>
        For Join, sign in on the source app and use <a
            href={resolve("/devices")}>Devices → Equal-Node joining</a
        > to inspect and approve the exact offer. Target setup codes never authorize
        source approval.
    </p>
    <form onsubmit={submit} class="flex flex-col gap-3">
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
                    onchange={clearSecrets}
                    disabled={session.busy || retrieving || !!session.intent}
                >
                    <option value="create">Create an Installation</option>
                    <option value="join">Join through a trusted peer</option>
                </select>
            {/snippet}
        </Field>
        {#each independentOwners as owner (owner)}
            {@const current = session.hosts.find(
                (host) => host.status.owners[0] === owner,
            )}
            <p>
                <strong>{owner}</strong>: {session.bases[owner]} — {current
                    ?.status.state}{session.outcomes[owner]
                    ? ` — ${session.outcomes[owner]}`
                    : ""}
            </p>
            {#if !bundled && !current?.status.ready}
                <Field label={`${owner} host setup code`}>
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
                            bind:value={codes[owner]}
                            autocomplete="off"
                            spellcheck="false"
                            pattern="[a-f0-9]{64}"
                            minlength="64"
                            maxlength="64"
                            disabled={session.busy || retrieving}
                        />
                    {/snippet}
                </Field>
            {/if}
        {/each}
        {#if purpose === "create"}
            <Field label="Administrator username" required>
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
                        bind:value={administrator}
                        autocomplete="username"
                        disabled={session.busy || retrieving || !!session.intent}
                    />
                {/snippet}
            </Field>
        {/if}
        {#if purpose === "join"}
            <Field label="Node name" required>
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
                        bind:value={node}
                        autocomplete="off"
                        disabled={session.busy || retrieving || !!session.intent}
                    />
                {/snippet}
            </Field>
        {/if}
        {#if purpose === "join"}
            <Field
                label="Trusted source Device HTTPS base#pinned endpoint"
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
                        bind:value={peer}
                        autocomplete="off"
                        spellcheck="false"
                        disabled={session.busy || retrieving || !!session.intent}
                        placeholder="https://source.example/device#…"
                    />
                {/snippet}
            </Field>
            <p>
                The target authenticates the pinned source and discovers its
                issuer. Do not supply an administrator, issuer or password for
                Join.
            </p>
        {:else if !session.hosts.find((host) => host.status.owners[0] === "idp")?.status.ready}
            <Field label="IdP administrator password" required>
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
                        disabled={session.busy || retrieving}
                    />
                {/snippet}
            </Field>
            <p>
                Only IdP receives this password. Re-enter the original password
                for an incomplete IdP Create retry.
            </p>
        {/if}
        {#if session.intent?.issuer}<p>
                Retained Installation issuer: {session.intent.issuer}
            </p>{/if}
        <button
            class="btn btn-primary btn-md"
            type="submit"
            disabled={!canSubmit}
            >{`${session.intent ? "Retry exact" : "Start"} ${purpose === "create" ? "Create" : "Join"} across owners`}</button
        >
        <button
            class="btn btn-secondary btn-md"
            type="button"
            onclick={recover}
            disabled={session.busy || retrieving || session.ready}
            >Recover exact operation across owners</button
        >
        <button
            class="btn btn-secondary btn-md"
            type="button"
            onclick={cancel}
            disabled={session.busy ||
                retrieving ||
                session.ready ||
                !session.intent ||
                !session.intent.issuer ||
                session.observationRequired}
            >Cancel exact operation across owners</button
        >
    </form>
    <button
        class="btn btn-secondary btn-md"
        type="button"
        onclick={observe}
        disabled={session.busy || retrieving}>Observe all owners and verify readiness</button
    >
    {#if session.message}
        {#if session.observationRequired}
            <Alert tone="danger">{session.message}</Alert>
        {:else}
            <Alert tone="info">{session.message}</Alert>
        {/if}
    {/if}
    {#if session.ready}<a
            class="btn btn-secondary btn-md"
            href={resolve("/signin")}>Continue to sign in</a
        >{/if}
</section>
