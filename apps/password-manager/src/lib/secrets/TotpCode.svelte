<script lang="ts">
import { untrack } from "svelte";

import { generateTotp, parseTotp, type TotpCode, type TotpParameters } from "./totp";

let { value }: { value: string } = $props();
let current = $state.raw<TotpCode | null>(null);
let error = $state("");
let reconcile = () => {};
let period = 30;
let copying = $state(false);
let feedback = $state("");
let copyFailed = $state(false);
let copyRevision = 0;

function clearCode() {
    current = null;
    copyRevision++;
    copying = false;
    feedback = "";
    copyFailed = false;
}

async function copy() {
    if (!current || copying) return;
    const now = Date.now();
    if (now >= current.expiresAt || now < current.expiresAt - period * 1000) {
        reconcile();
        feedback = "Code changed. Copy the updated code.";
        return;
    }
    const request = ++copyRevision;
    copying = true;
    feedback = "";
    try {
        if (!navigator.clipboard?.writeText) throw new Error();
        await navigator.clipboard.writeText(current.code);
        if (request === copyRevision) {
            copyFailed = false;
            feedback = "Copied. Clipboard is not cleared automatically.";
        }
    } catch {
        if (request === copyRevision) {
            copyFailed = true;
            feedback = "Copy failed. Clipboard access is unavailable or denied. Copy the code manually.";
        }
    } finally {
        if (request === copyRevision) copying = false;
    }
}

function activate() {
    let parameters: TotpParameters | undefined;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let active = true;
    let revision = 0;
    const input = value;
    untrack(() => { clearCode(); error = ""; });
    try {
        parameters = parseTotp(input);
        period = parameters.period;
    } catch (cause) {
        error = (cause as Error).message;
    }

    async function update() {
        const request = ++revision;
        clearTimeout(timer);
        if (!active || !parameters) return;
        if (document.visibilityState === "hidden") {
            clearCode();
            return;
        }
        const now = Date.now();
        if (current && (now >= current.expiresAt || now < current.expiresAt - parameters.period * 1000)) clearCode();
        try {
            const result = await generateTotp(parameters, now);
            if (!active || request !== revision) return;
            const completedAt = Date.now();
            if (completedAt >= result.expiresAt || completedAt < result.expiresAt - parameters.period * 1000) {
                void update();
                return;
            }
            result.remaining = Math.ceil((result.expiresAt - completedAt) / 1000);
            current = result;
            error = "";
        } catch {
            if (!active || request !== revision) return;
            clearCode();
            error = "TOTP generation failed. Local cryptography is unavailable.";
            return;
        }
        timer = setTimeout(() => void update(), 1000 - Date.now() % 1000);
    }
    reconcile = () => { clearCode(); void update(); };
    untrack(() => void update());
    return () => {
        active = false;
        revision++;
        clearTimeout(timer);
        parameters?.secret.fill(0);
        parameters = undefined;
        clearCode();
        error = "";
        reconcile = () => {};
    };
}
</script>

<svelte:document onvisibilitychange={() => reconcile()} />
<svelte:window onfocus={() => reconcile()} onpageshow={() => reconcile()} />

<section aria-label="Authenticator code" class="flex flex-col gap-2" {@attach activate}>
    {#if error}
        <p role="alert">{error}</p>
    {:else if current}
        <p class="mb-0 font-mono" aria-label="TOTP code">{current.code}</p>
        <button class="btn btn-secondary btn-md" type="button" disabled={copying} onclick={() => void copy()}>Copy TOTP code</button>
        <p role="timer" aria-label="TOTP time remaining">{current.remaining} {current.remaining === 1 ? "second" : "seconds"} remaining</p>
    {:else}
        <p>TOTP code unavailable while loading or the tab is hidden.</p>
    {/if}
    {#if feedback}<p role={copyFailed ? "alert" : "status"}>{feedback}</p>{/if}
    <p class="text-sm">Generated locally using this device's clock. This does not add a factor to local vault unlock.</p>
</section>
