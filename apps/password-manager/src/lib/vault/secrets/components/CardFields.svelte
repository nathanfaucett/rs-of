<script lang="ts">
import { Field, type FieldControl } from "$lib/ui";
import type { CardDraft } from "../secret-draft";
import PasswordInput from "./PasswordInput.svelte";

let { draft, disabled }: { draft: CardDraft; disabled: boolean } = $props();
</script>

<fieldset class="flex min-w-0 flex-col gap-3" {disabled}>
    <legend class="text-sm font-medium">Card</legend>
    <p>All payment fields are optional. Values are stored as entered, including expired dates and unknown brands.</p>
    <Field id="cardholderName" label="Cardholder name">
        {#snippet children(control: FieldControl)}
            <input {...control} bind:value={draft.cardholderName} autocomplete="off" />
        {/snippet}
    </Field>
    <Field id="brand" label="Brand">
        {#snippet children(control: FieldControl)}
            <input {...control} bind:value={draft.brand} autocomplete="off" />
        {/snippet}
    </Field>
    <Field id="number" label="Card number">
        {#snippet children(control: FieldControl)}
            <PasswordInput {...control} label="Card number" bind:value={draft.number} />
        {/snippet}
    </Field>
    <Field id="expMonth" label="Expiration month">
        {#snippet children(control: FieldControl)}
            <input {...control} bind:value={draft.expMonth} autocomplete="off" />
        {/snippet}
    </Field>
    <Field id="expYear" label="Expiration year">
        {#snippet children(control: FieldControl)}
            <input {...control} bind:value={draft.expYear} autocomplete="off" />
        {/snippet}
    </Field>
    <Field id="code" label="Security code">
        {#snippet children(control: FieldControl)}
            <PasswordInput {...control} label="Security code" bind:value={draft.code} />
        {/snippet}
    </Field>
</fieldset>
