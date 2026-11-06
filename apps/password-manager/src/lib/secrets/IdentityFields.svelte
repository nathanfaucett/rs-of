<script lang="ts">
import { Field } from "$lib/ui";
import { identityFields } from "./identity-fields";
import type { IdentityDraft } from "./secret-draft";

let { draft = $bindable(), disabled }: { draft: IdentityDraft; disabled: boolean } = $props();
</script>

<fieldset class="flex min-w-0 flex-col gap-3" {disabled}>
    <legend class="text-sm font-medium">Identity</legend>
    <p>All Identity fields are optional. Values are stored as entered.</p>
    {#each identityFields as [key, label] (key)}
        <Field id={`identity-${key}`} {label}>
            {#snippet children(control)}
                {#if key === "ssn" || key === "passportNumber" || key === "licenseNumber"}
                    <input {...control} bind:value={draft[key]} type="password" autocomplete="off" spellcheck="false" />
                {:else}
                    <textarea {...control} bind:value={draft[key]} rows={key.startsWith("address") ? 2 : 1} autocomplete="off" spellcheck="false"></textarea>
                {/if}
            {/snippet}
        </Field>
    {/each}
</fieldset>
