<script lang="ts">
import { Card } from "$lib/ui";
import type { Secret } from "../secret";
import ProtectedValue from "./ProtectedValue.svelte";
import { identityFields } from "../identity-fields";
import TotpCode from "./TotpCode.svelte";

let { record, canEdit, onedit, onclose }: {
    record: Secret;
    canEdit: boolean;
    onedit: () => void;
    onclose: () => void;
} = $props();
</script>

<Card title="Item details" description="Read-only. Use Edit to change this item.">
    {#snippet actions()}
        <button class="btn btn-primary btn-md" type="button" disabled={!canEdit} onclick={onedit}>Edit</button>
        <button class="btn btn-secondary btn-md" type="button" disabled={!canEdit} onclick={onclose}>Close details</button>
    {/snippet}
    <div class="flex flex-col gap-4">
        <h2 tabindex="-1" data-detail-heading {@attach (element) => { element.focus(); }}>{record.name}</h2>
        <p>Type: {record.kind}</p>
        <p>Favorite: {record.favorite ? "Yes" : "No"}</p>
        {#if record.kind === "login"}
            <ProtectedValue label="Username" value={record.username} protected={false} />
            <ProtectedValue label="Password" value={record.secret} />
            <ProtectedValue label="Authenticator key (TOTP)" value={record.totp} />
            {#if record.totp}<TotpCode value={record.totp} />{/if}
            <h3>Websites</h3>
            {#each record.uris as uri, i (uri)}
                <ProtectedValue label={`Website ${i + 1}`} value={uri.uri} protected={false} />
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
            <p>Password changed at <time datetime={entry.changedAt}>{entry.changedAt}</time></p>
            <ProtectedValue label={`History password ${i + 1}`} value={entry.secret} />
        {:else}<p>No password history.</p>{/each}
        <p>Created: <time datetime={record.createdAt}>{record.createdAt}</time></p>
        <p>Updated: <time datetime={record.updatedAt}>{record.updatedAt}</time></p>
    </div>
</Card>
