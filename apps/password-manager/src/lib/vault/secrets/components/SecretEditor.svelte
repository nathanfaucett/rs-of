<script lang="ts">
import { onDestroy, untrack } from "svelte";
import { beforeNavigate } from "$app/navigation";
import { vaultStateModule } from "$lib/shared/state/vault.svelte";
import { Alert, Card, Field, type FieldControl } from "$lib/ui";
import type { VaultHandleLike } from "$lib/vault/types";
import GeneratorDialog from "../generator/GeneratorDialog.svelte";
import type { EditableKind } from "../secret-draft";
import { createLocalSecretWorkflow } from "../workflow.svelte";
import CardFields from "./CardFields.svelte";
import IdentityFields from "./IdentityFields.svelte";
import PasswordInput from "./PasswordInput.svelte";
import SshKeyFields from "./SshKeyFields.svelte";

interface Props {
    vault: VaultHandleLike;
    kind?: EditableKind;
    recordId?: string | null;
    onExit: () => void;
}

let { vault, kind = "login", recordId, onExit }: Props = $props();

const sessionVault = untrack(() => vault);
const workflow = createLocalSecretWorkflow({ vault: sessionVault });

function isCurrentSession() {
    return active && vaultStateModule.vaultData?.vault === sessionVault;
}

beforeNavigate((navigation) => {
    if (!isCurrentSession()) return;
    if (navigation.willUnload) {
        if (workflow.dirty) navigation.cancel();
    } else if (workflow.busy || (workflow.dirty && !window.confirm("Discard unsaved changes?"))) {
        navigation.cancel();
    }
});
let active = true;
onDestroy(() => {
    active = false;
    workflow.dispose();
});

let error = $state("");
let initialized = false;

$effect(() => {
    if (initialized || workflow.loading || workflow.loadError) return;
    initialized = true;
    // Load from the ready collection once so later reads cannot replace a draft.
    untrack(() => {
        const item = recordId ? workflow.records.find((value) => value.id === recordId) : null;
        if (recordId && !item) error = "Item not found.";
        else workflow.edit(item ?? null, kind);
    });
});

async function handleSave() {
    if (!isCurrentSession() || !workflow.editor || vaultStateModule.vaultData?.inactivity.check()) return;
    const saved = await workflow.save(workflow.editor.draft);
    if (saved && isCurrentSession() && !vaultStateModule.vaultData?.inactivity.check()) {
        onExit();
    }
}

</script>

{#if error || workflow.loadError}
    <Alert tone="danger">{error || workflow.loadError}</Alert>
{:else if workflow.loading}
    <div class="flex items-center justify-center h-64">
        <p class="text-muted-foreground">Loading item...</p>
    </div>
{:else}
    {#if workflow.editor}
        {@const editor = workflow.editor}
        {@const draft = editor.draft}
        <Card>
            <form class="flex flex-col gap-4" onsubmit={(event) => { event.preventDefault(); void handleSave(); }}>
                <Field id="name" label="Name" required>
                    {#snippet children(control: FieldControl)}
                        <input {...control} {@attach (element) => element?.focus()} bind:value={draft.name} autocomplete="off" disabled={!workflow.canEdit} />
                    {/snippet}
                </Field>
                {#if draft.kind === "login"}
                    {@const loginDraft = draft as import("../secret-draft").LoginDraft}
                    <Field id="username" label="Username">
                        {#snippet children(control: FieldControl)}
                            <input {...control} bind:value={loginDraft.username} autocomplete="off" disabled={!workflow.canEdit} />
                        {/snippet}
                    </Field>
                    <Field id="secret" label="Password">
                        {#snippet children(control: FieldControl)}
                            <div class="flex flex-wrap gap-2">
                                <PasswordInput {...control} label="Password" bind:value={loginDraft.secret} autocomplete="new-password" disabled={!workflow.canEdit} />
                                <GeneratorDialog onuse={(value) => { if (workflow.canEdit && editor.draft.kind === "login") { editor.draft.secret = value; editor.confirmPassword = value; } }} />
                            </div>
                        {/snippet}
                    </Field>
                    {#if loginDraft.secret}
                        <Field id="confirm-secret" label="Confirm password" required>
                            {#snippet children(control: FieldControl)}
                                <PasswordInput {...control} label="Confirm password" bind:value={editor.confirmPassword} autocomplete="new-password" disabled={!workflow.canEdit} />
                            {/snippet}
                        </Field>
                    {/if}
                    <Field id="totp" label="Authenticator key (TOTP)" description="Store a Base32 key or otpauth TOTP URI. View local codes in item details.">
                        {#snippet children(control: FieldControl)}
                            <PasswordInput {...control} label="Authenticator key (TOTP)" bind:value={draft.totp} disabled={!workflow.canEdit} />
                        {/snippet}
                    </Field>
                    <fieldset class="flex min-w-0 flex-col gap-3" disabled={!workflow.canEdit}>
                        <legend class="text-sm font-medium">Websites</legend>
                        {#each draft.uris as uri, i (uri)}
                            <div class="flex flex-wrap items-end gap-2">
                                <Field id={`uri-${i}`} label={`Website ${i + 1}`}>
                                    {#snippet children(control: FieldControl)}
                                        <input {...control} bind:value={uri.uri} placeholder="https://example.com" />
                                    {/snippet}
                                </Field>
                                <Field id={`match-${i}`} label={`Match rule ${i + 1}`}>
                                    {#snippet children(control: FieldControl)}
                                        <select {...control} bind:value={uri.match}>
                                            <option value={null}>No rule</option>
                                            <option value="domain">Domain</option>
                                            <option value="host">Host</option>
                                            <option value="startsWith">Starts with</option>
                                            <option value="exact">Exact</option>
                                            <option value="regex">Regex</option>
                                            <option value="never">Never</option>
                                        </select>
                                    {/snippet}
                                </Field>
                                <button class="btn btn-ghost btn-md text-danger-text" type="button" aria-label={`Remove website ${i + 1}`} onclick={() => draft.uris.splice(i, 1)}>
                                    <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
                                </button>
                            </div>
                        {/each}
                        <button class="btn btn-secondary btn-md self-start" type="button" onclick={() => draft.uris.push({ uri: "", match: null })}>
                            <svg class="w-4 h-4 mr-1" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></svg>Add website
                        </button>
                    </fieldset>
                {:else if draft.kind === "card"}
                    <CardFields {draft} disabled={!workflow.canEdit} />
                {:else if draft.kind === "identity"}
                    <IdentityFields {draft} disabled={!workflow.canEdit} />
                {:else if draft.kind === "sshKey"}
                    <SshKeyFields bind:draft={editor.draft as import("../secret-draft").SshKeyDraft} disabled={!workflow.canEdit} />
                {:else if draft.kind === "secureNote"}
                    <!-- Secure notes only have name, notes, and custom fields -->
            {/if}
            <fieldset class="flex min-w-0 flex-col gap-3" disabled={!workflow.canEdit}>
                <legend class="text-sm font-medium">Custom fields</legend>
                {#each draft.fields as field, i (field)}

                    <div class="flex flex-wrap items-end gap-2">
                        <Field id={`field-name-${i}`} label={`Field name ${i + 1}`} required>
                            {#snippet children(control: FieldControl)}
                                <input {...control} bind:value={field.name} />
                            {/snippet}
                        </Field>
                        <Field id={`field-type-${i}`} label={`Field type ${i + 1}`}>
                            {#snippet children(control: FieldControl)}
                                <select {...control} bind:value={() => field.type, (type) => {
                                    if (field.type === "hidden" && type !== "hidden") field.value = "";
                                    field.type = type;
                                }}>
                                    <option value="text">Text</option>
                                    <option value="hidden">Hidden</option>
                                    <option value="boolean">Boolean</option>
                                </select>
                            {/snippet}
                        </Field>
                        <Field id={`field-value-${i}`} label={`Field value ${i + 1}`}>
                            {#snippet children(control: FieldControl)}
                                {#if field.type === "boolean"}
                                    <select {...control} bind:value={field.value}>
                                        {#if field.value !== "true" && field.value !== "false"}
                                            <option value={field.value} disabled>{field.value || "Select True or False"}</option>
                                        {/if}
                                        <option value="true">True</option>
                                        <option value="false">False</option>
                                    </select>
                                {:else if field.type === "hidden"}
                                    <PasswordInput {...control} label={`Field value ${i + 1}`} bind:value={field.value} />
                                {:else}
                                    <input {...control} bind:value={field.value} type="text" autocomplete="off" />
                                {/if}
                            {/snippet}
                        </Field>
                        <button class="btn btn-ghost btn-md text-danger-text" type="button" aria-label={`Remove custom field ${i + 1}`} onclick={() => draft.fields.splice(i, 1)}>
                            <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
                        </button>
                    </div>
                {/each}
                <button class="btn btn-secondary btn-md self-start" type="button" onclick={() => draft.fields.push({ name: "", value: "", type: "text" })}>
                    <svg class="w-4 h-4 mr-1" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></svg>Add custom field
                </button>
            </fieldset>
            <Field id="notes" label="Notes">
                {#snippet children(control: FieldControl)}
                    {#if draft.kind === "secureNote" || draft.kind === "identity"}
                        <PasswordInput {...control} label="Notes" bind:value={draft.notes} rows={3} disabled={!workflow.canEdit} />
                    {:else}
                        <textarea {...control} bind:value={draft.notes} rows="3" disabled={!workflow.canEdit}></textarea>
                    {/if}
                {/snippet}
            </Field>
            <label class="flex items-center gap-2 text-sm text-ink" for="favorite">
                <input id="favorite" bind:checked={draft.favorite} type="checkbox" disabled={!workflow.canEdit} />
                <span>Favorite</span>
            </label>
            {#if workflow.error}
                <Alert tone="danger" title="Error">{workflow.error}</Alert>
            {/if}
            {#if workflow.notice}
                <Alert tone="success">{workflow.notice}</Alert>
            {/if}
            <div class="flex gap-2">
                <button class="btn btn-primary btn-md" type="submit" disabled={!workflow.canEdit || workflow.busy}>
                    {workflow.busy ? "Saving..." : (editor.record ? "Save changes" : "Create item")}
                </button>
                <button class="btn btn-secondary btn-md" type="button" disabled={!workflow.canEdit} onclick={onExit}>
                    Cancel
                </button>
            </div>
            </form>
        </Card>
    {:else}
        <div class="flex items-center justify-center h-64">
            <p class="text-muted-foreground">Initializing editor...</p>
        </div>
    {/if}
{/if}
