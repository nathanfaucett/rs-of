<script lang="ts">
import { Card } from "$lib/ui";
import type { Secret } from "./secret";
import ProtectedValue from "./ProtectedValue.svelte";

let { record, canEdit, onedit, onclose }: {
    record: Secret;
    canEdit: boolean;
    onedit: () => void;
    onclose: () => void;
} = $props();
</script>

<Card title="Item details" description="Read-only. Use Edit to change this item.">
    {#snippet actions()}
        {#if record.kind === "login" || record.kind === "secureNote"}
            <button class="btn btn-primary btn-md" type="button" disabled={!canEdit} onclick={onedit}>Edit</button>
        {/if}
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
            <h3>Websites</h3>
            {#each record.uris as uri, i (uri)}
                <ProtectedValue label={`Website ${i + 1}`} value={uri.uri} protected={false} />
                <p>Match rule: {uri.match ?? "Default (inherited)"}</p>
            {:else}<p>No websites.</p>{/each}
        {:else if record.kind === "secureNote"}
            <p>Secure Note: private plain text.</p>
        {:else}
            <p>Type-specific details and editing are not available yet.</p>
        {/if}
        <ProtectedValue label="Notes" value={record.notes} protected={record.kind === "secureNote"} />
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
