<script lang="ts">
import { Field } from "$lib/ui";
import type { SshKeyDraft } from "./secret-draft";

let { draft = $bindable(), disabled }: {
    draft: SshKeyDraft;
    disabled: boolean;
} = $props();
let revealed = $state(false);
</script>

<p>Store key text only. Keys and fingerprints are not verified. No key generation or SSH agent is provided.</p>
<button class="btn btn-secondary btn-md self-start" type="button" {disabled}
    aria-pressed={revealed} onclick={() => revealed = !revealed}>{revealed ? "Hide" : "Show"} Private key</button>
{#if revealed}
    <Field id="private-key" label="Private key" required>
        {#snippet children(control)}
            <textarea {...control} bind:value={draft.privateKey} rows="6" autocomplete="off" spellcheck="false" autocapitalize="off" {disabled}></textarea>
        {/snippet}
    </Field>
{:else}
    <p>Private key is hidden. Select Show Private key to enter or change it.</p>
{/if}
<Field id="public-key" label="Public key">
    {#snippet children(control)}
        <textarea {...control} bind:value={draft.publicKey} rows="3" autocomplete="off" spellcheck="false" autocapitalize="off" {disabled}></textarea>
    {/snippet}
</Field>
<Field id="key-fingerprint" label="Key fingerprint">
    {#snippet children(control)}
        <input {...control} bind:value={draft.keyFingerprint} autocomplete="off" spellcheck="false" autocapitalize="off" {disabled} />
    {/snippet}
</Field>
