<script lang="ts">
import { Field } from "$lib/ui";
import PasswordInput from "./PasswordInput.svelte";
import type { CardDraft } from "../secret-draft";

let { draft, disabled }: { draft: CardDraft; disabled: boolean } = $props();
</script>

<fieldset class="flex min-w-0 flex-col gap-3" {disabled}>
    <legend class="text-sm font-medium">Card</legend>
    <p>All payment fields are optional. Values are stored as entered, including expired dates and unknown brands.</p>
    <Field id="cardholderName" label="Cardholder name">
        {#snippet children(control)}
            <input {...control} bind:value={draft.cardholderName} autocomplete="off" />
        {/snippet}
    </Field>
    <Field id="brand" label="Brand">
        {#snippet children(control)}
            <input {...control} bind:value={draft.brand} autocomplete="off" />
        {/snippet}
    </Field>
    <Field id="number" label="Card number">
        {#snippet children(control)}
            <PasswordInput {...control} bind:value={draft.number} autocomplete="off" spellcheck="false" />
        {/snippet}
    </Field>
    <Field id="expMonth" label="Expiration month">
        {#snippet children(control)}
            <input {...control} bind:value={draft.expMonth} autocomplete="off" />
        {/snippet}
    </Field>
    <Field id="expYear" label="Expiration year">
        {#snippet children(control)}
            <input {...control} bind:value={draft.expYear} autocomplete="off" />
        {/snippet}
    </Field>
    <Field id="code" label="Security code">
        {#snippet children(control)}
            <PasswordInput {...control} bind:value={draft.code} autocomplete="off" spellcheck="false" />
        {/snippet}
    </Field>
</fieldset>
