<script lang="ts">
import { onDestroy } from "svelte";
import { beforeNavigate } from "$app/navigation";
import ProtectedValue from "../components/ProtectedValue.svelte";
import { Alert, Card, Field } from "$lib/ui";
import wordlistLicense from "./WORDLIST-LICENSE?raw";
import { generatePassword, generatePassphrase, validatePasswordSettings, validatePassphraseSettings } from "./generator";

let { onuse }: { onuse?: (value: string) => void } = $props();
const id = $props.id();
let mode = $state("password");
let password = $state({ length: 20, lowercase: true, uppercase: true, digits: true, symbols: true, excludeAmbiguous: false });
let passphrase = $state({ words: 6, separator: "-", capitalize: false, appendNumber: false });
let value = $state<string | null>(null);
let error = $state("");
const validation = $derived(mode === "password" ? validatePasswordSettings(password) : validatePassphraseSettings(passphrase));

function clear() { value = null; error = ""; }
onDestroy(clear);
beforeNavigate(clear);
function generate() {
    clear();
    try {
        value = mode === "password" ? generatePassword(password) : generatePassphrase(passphrase);
    } catch {
        error = "Generation failed. Check settings and use a browser with secure random generation.";
    }
}
</script>

<svelte:window onpagehide={clear} />

<Card title="Password and passphrase generator" description="Generated locally. Nothing is saved until you save an item.">
    <div class="flex flex-col gap-4">
        <fieldset class="flex min-w-0 flex-col gap-3" oninput={clear} onchange={clear}>
            <legend class="text-sm font-medium">Generator settings</legend>
            <Field id={`${id}-mode`} label="Generate type">
                {#snippet children(control)}
                    <select {...control} bind:value={mode} {@attach (element) => { element.focus(); }}>
                        <option value="password">Password</option>
                        <option value="passphrase">Passphrase</option>
                    </select>
                {/snippet}
            </Field>
            {#if mode === "password"}
                <Field id={`${id}-length`} label="Password length" description="8–128 characters. Each selected class appears at least once.">
                    {#snippet children(control)}<input {...control} type="number" min="8" max="128" step="1" bind:value={password.length} />{/snippet}
                </Field>
                <label><input type="checkbox" bind:checked={password.lowercase} /> Lowercase (a–z)</label>
                <label><input type="checkbox" bind:checked={password.uppercase} /> Uppercase (A–Z)</label>
                <label><input type="checkbox" bind:checked={password.digits} /> Digits (0–9)</label>
                <label><input type="checkbox" bind:checked={password.symbols} /> Symbols (!@#$%&*?+-=_.)</label>
                <label><input type="checkbox" bind:checked={password.excludeAmbiguous} /> Exclude ambiguous characters (I l 1 O 0 o)</label>
            {:else}
                <Field id={`${id}-words`} label="Word count" description="4–20 words from the bundled 2,048-word English list. Words can repeat.">
                    {#snippet children(control)}<input {...control} type="number" min="4" max="20" step="1" bind:value={passphrase.words} />{/snippet}
                </Field>
                <Field id={`${id}-separator`} label="Word separator">
                    {#snippet children(control)}
                        <select {...control} bind:value={passphrase.separator}>
                            <option value="-">Hyphen</option><option value=" ">Space</option><option value=".">Period</option><option value="_">Underscore</option>
                        </select>
                    {/snippet}
                </Field>
                <label><input type="checkbox" bind:checked={passphrase.capitalize} /> Capitalize each word</label>
                <label><input type="checkbox" bind:checked={passphrase.appendNumber} /> Append one random digit</label>
            {/if}
        </fieldset>
        {#if validation}<p role="alert">{validation}</p>{/if}
        {#if error}<Alert tone="danger">{error}</Alert>{/if}
        <button class="btn btn-secondary btn-md self-start" type="button" disabled={!!validation} onclick={generate}>Generate</button>
        {#key value}
            <ProtectedValue label="Generated value" {value} />
        {/key}
        {#if onuse}
            <p>Use replaces the current Login draft password and confirmation. It does not save the item.</p>
            <button class="btn btn-primary btn-md self-start" type="button" disabled={value === null}
                onclick={() => { if (value !== null) { onuse?.(value); clear(); } }}>Use in Login draft</button>
        {:else}
            <p>Open a Login draft to insert a generated value.</p>
        {/if}
        <button class="btn btn-ghost btn-md self-start" type="button" disabled={value === null} onclick={clear}>Clear generated value</button>
        <details>
            <summary>Word list source and license</summary>
            <p>BIP-39 English, 2,048 words. Source: trezor/python-mnemonic, revision b57a5ad77a981e743f4167ab2f7927a55c1e82a8. This generator does not create wallet mnemonics.</p>
            <pre class="whitespace-pre-wrap text-sm">{wordlistLicense}</pre>
        </details>
    </div>
</Card>
