<script lang="ts">
import { Field, type FieldControl } from "$lib/ui";
import { identityFields } from "../identity-fields";
import type { IdentityDraft } from "../secret-draft";
import PasswordInput from "./PasswordInput.svelte";

let { draft = $bindable(), disabled }: { draft: IdentityDraft; disabled: boolean } = $props();
</script>

<fieldset class="flex min-w-0 flex-col gap-3" {disabled}>
    <legend class="text-sm font-medium">Identity</legend>
    <p>All Identity fields are optional. Values are stored as entered.</p>
    {#each identityFields as [key, label] (key)}
        <Field id={`identity-${key}`} {label}>
            {#snippet children(control: FieldControl)}
                <PasswordInput {...control} {label} bind:value={draft[key]}
                    rows={key === "ssn" || key === "passportNumber" || key === "licenseNumber" ? undefined : key.startsWith("address") ? 2 : 1} />
            {/snippet}
        </Field>
    {/each}
</fieldset>
