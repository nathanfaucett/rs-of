<script lang="ts">
import { Field, type FieldControl } from "$lib/ui";
import type { SshKeyDraft } from "../secret-draft";
import PasswordInput from "./PasswordInput.svelte";

let { draft = $bindable(), disabled }: {
    draft: SshKeyDraft;
    disabled: boolean;
} = $props();
</script>

<p>Store key text only. Keys and fingerprints are not verified. No key generation or SSH agent is provided.</p>
<Field id="private-key" label="Private key" required>
    {#snippet children(control: FieldControl)}
        <PasswordInput {...control} label="Private key" bind:value={draft.privateKey} rows={6} {disabled} />
    {/snippet}
</Field>
<Field id="public-key" label="Public key">
    {#snippet children(control: FieldControl)}
        <textarea {...control} bind:value={draft.publicKey} rows="3" autocomplete="off" spellcheck="false" autocapitalize="off" {disabled}></textarea>
    {/snippet}
</Field>
<Field id="key-fingerprint" label="Key fingerprint">
    {#snippet children(control: FieldControl)}
        <input {...control} bind:value={draft.keyFingerprint} autocomplete="off" spellcheck="false" autocapitalize="off" {disabled} />
    {/snippet}
</Field>
