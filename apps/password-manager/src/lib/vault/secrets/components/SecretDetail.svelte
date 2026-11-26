<script lang="ts">
import type { Snippet } from "svelte";
import { formatTimestamp } from "$lib/shared/util/timestamp";
import { Card } from "$lib/ui";
import { identityFields } from "../identity-fields";
import type { Secret } from "../secret";
import ProtectedValue from "./ProtectedValue.svelte";
import TotpCode from "./TotpCode.svelte";

let { record, canEdit, onedit, onclose, folderAssignment }: {
    record: Secret;
    canEdit: boolean;
    onedit: () => void;
    onclose: () => void;
    folderAssignment: Snippet;
} = $props();

function websiteHref(value: string): string | undefined {
    if (!/^https?:\/\/[^/?#]/i.test(value) || /[\s\\\p{Cc}]/u.test(value)) return undefined;
    try {
        return new URL(value).href;
    } catch {
        return undefined;
    }
}
</script>

{#snippet timestamp(value: string)}
    {@const formatted = formatTimestamp(value)}
    {#if formatted}<time datetime={value}>{formatted}</time>{:else}Unknown{/if}
{/snippet}

<Card title="Item details" description="Use Edit to change item fields.">
    <div class="flex min-w-0 flex-col gap-4 wrap-anywhere">
        <h2 tabindex="-1" {@attach (element) => { element.focus(); }}>{record.name}</h2>
        <div class="flex flex-wrap gap-2">
            <button class="btn btn-primary btn-md" type="button" disabled={!canEdit} onclick={onedit}>Edit</button>
            <button class="btn btn-secondary btn-md" type="button" disabled={!canEdit} onclick={onclose}>Close details</button>
        </div>
        {@render folderAssignment()}
        <p>Type: {record.kind}</p>
        <p>Favorite: {record.favorite ? "Yes" : "No"}</p>
        {#if record.kind === "login"}
            <ProtectedValue label="Username" value={record.username} protected={false} />
            <ProtectedValue label="Password" value={record.secret} />
            <ProtectedValue label="Authenticator key (TOTP)" value={record.totp} />
            {#if record.totp}<TotpCode value={record.totp} />{/if}
            <h3>Websites</h3>
            {#each record.uris as uri, i (uri)}
                {@const href = websiteHref(uri.uri)}
                <ProtectedValue label={`Website ${i + 1}`} value={uri.uri} protected={false}>
                    {#snippet actions()}
                        {#if href}
                            <a class="btn btn-secondary btn-md" {href} target="_blank" rel="noopener noreferrer"
                                aria-label={`Launch ${uri.uri}`}>Launch</a>
                        {/if}
                    {/snippet}
                </ProtectedValue>
                <p>Match rule: {uri.match ?? "Default (inherited)"}</p>
            {:else}<p>No websites.</p>{/each}
        {:else if record.kind === "card"}
            <ProtectedValue label="Cardholder name" value={record.cardholderName} protected={false} />
            <ProtectedValue label="Brand" value={record.brand} protected={false} />
            <ProtectedValue label="Card number" value={record.number} />
            <ProtectedValue label="Expiration month" value={record.expMonth} protected={false} />
            <ProtectedValue label="Expiration year" value={record.expYear} protected={false} />
            <ProtectedValue label="Security code" value={record.code} />
        {:else if record.kind === "identity"}
            {#each identityFields as [key, label] (key)}
                <ProtectedValue {label} value={record[key]} />
            {/each}
        {:else if record.kind === "sshKey"}
            <ProtectedValue label="Private key" value={record.privateKey} />
            <ProtectedValue label="Public key" value={record.publicKey} protected={false} />
            <ProtectedValue label="Key fingerprint" value={record.keyFingerprint} protected={false} />
            <p>Stored key text only. Key pairs and fingerprints are not verified.</p>
        {:else if record.kind === "secureNote"}
            <p>Secure Note: private plain text.</p>
        {/if}
        <ProtectedValue label="Notes" value={record.notes} protected={record.kind === "secureNote" || record.kind === "identity"} />
        <h3>Custom fields</h3>
        {#each record.fields as field, i (field)}
            <ProtectedValue label={`Custom field ${i + 1}: ${field.name}`} value={field.value} protected={field.type === "hidden"} />
            <p>Field type: {field.type}</p>
        {:else}<p>No custom fields.</p>{/each}
        <h3>Password history ({record.history.length})</h3>
        {#each record.history as entry, i (entry)}
            <p>Password changed at {@render timestamp(entry.changedAt)}</p>
            <ProtectedValue label={`History password ${i + 1}`} value={entry.secret} />
        {:else}<p>No password history.</p>{/each}
        <p>Created: {@render timestamp(record.createdAt)}</p>
        <p>Updated: {@render timestamp(record.updatedAt)}</p>
    </div>
</Card>
