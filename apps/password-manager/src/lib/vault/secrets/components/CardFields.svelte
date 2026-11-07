<script lang="ts">
import { Field } from "$lib/ui";
import type { CardDraft } from "../secret-draft";

let { draft, disabled }: { draft: CardDraft; disabled: boolean } = $props();
let showNumber = $state(false);
let showCode = $state(false);
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
            <input {...control} bind:value={draft.number} type={showNumber ? "text" : "password"} autocomplete="off" spellcheck="false" />
        {/snippet}
    </Field>
    <button class="btn btn-secondary btn-md self-start" type="button" aria-controls="number" aria-pressed={showNumber}
        onclick={() => showNumber = !showNumber}>{showNumber ? "Hide card number" : "Show card number"}</button>
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
            <input {...control} bind:value={draft.code} type={showCode ? "text" : "password"} autocomplete="off" spellcheck="false" />
        {/snippet}
    </Field>
    <button class="btn btn-secondary btn-md self-start" type="button" aria-controls="code" aria-pressed={showCode}
        onclick={() => showCode = !showCode}>{showCode ? "Hide security code" : "Show security code"}</button>
</fieldset>
