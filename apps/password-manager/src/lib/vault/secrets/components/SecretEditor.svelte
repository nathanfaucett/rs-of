<script lang="ts">
import { onMount } from "svelte";
import CardFields from "./CardFields.svelte";
import IdentityFields from "./IdentityFields.svelte";
import SshKeyFields from "./SshKeyFields.svelte";
import GeneratorDialog from "../generator/GeneratorDialog.svelte";
import PasswordInput from "./PasswordInput.svelte";
import { Alert, Card, Field } from "$lib/ui";
import type { VaultHandle } from "../../vault";
import { createLocalSecretWorkflow } from "../workflow.svelte";
import type { Secret } from "../secret";
import {
    createSecretDraft,
    type EditableKind,
    type EditableSecret,
    type SecretDraft,
} from "../secret-draft";
import * as v from "valibot";
import {
    CardSchema,
    IdentitySchema,
    LoginSchema,
    SecureNoteSchema,
    SshKeySchema,
} from "../secret";
import { SvelteDate } from "svelte/reactivity";
import { UnacknowledgedStorageWriteError } from "../collection";
import { StaleItemMutationError } from "../../folder";

interface Props {
    vault: VaultHandle;
    kind?: EditableKind;
    record?: EditableSecret | null;
    recordId?: string | null;
    onSave: () => void;
    onCancel: () => void;
}

let { vault, kind = "login", record, recordId, onSave, onCancel }: Props = $props();

const workflow = $state.raw(createLocalSecretWorkflow({ vault }));

let error = $state("");
let notice = $state("");
let loading = $state(false);

async function loadRecord() {
    if (!recordId) return;
    loading = true;
    try {
        // Wait for workflow to be ready
        await new Promise<void>((resolve) => {
            const checkReady = () => {
                if (!workflow.loading || workflow.loadError) {
                    resolve();
                } else {
                    setTimeout(checkReady, 50);
                }
            };
            checkReady();
        });

        // Get the record from the collection via workflow
        const records = workflow.records;
        const found = records.find((r) => r.id === recordId);
        if (found && !found.deletedAt) {
            workflow.edit(found);
        } else {
            error = "Record not found";
        }
    } catch (e) {
        error = e instanceof Error ? e.message : "Failed to load record";
    } finally {
        loading = false;
    }
}

$effect(() => {
    if (record) {
        workflow.edit(record);
    } else if (recordId) {
        loadRecord();
    } else {
        workflow.edit(null, kind);
    }
});

async function handleSave() {
    if (!workflow.editor) return;
    const saved = await workflow.save(workflow.editor.draft);
    if (saved) {
        onSave();
    }
}

function handleCancel() {
    workflow.cancel();
    onCancel();
}

function customFieldIssue(editorRecord: EditableSecret | null, fields: SecretDraft["fields"], index: number) {
    // Simplified - just return undefined for now
    return undefined;
}
</script>

{#if loading}
    <div class="flex items-center justify-center h-64">
        <p class="text-muted-foreground">Loading secret...</p>
    </div>
{:else if workflow.editor}
    {@const editor = workflow.editor}
    {@const draft = editor.draft}
    <Card title={editor.record ? "Edit secret" : "New secret"} description="Changes are encrypted and stored when you save.">
        <form class="flex flex-col gap-4" onsubmit={(event) => { event.preventDefault(); handleSave(); }}>
            <Field id="name" label="Name" required>
                {#snippet children(control)}
                    <input {...control} {@attach (element) => element?.focus()} bind:value={draft.name} autocomplete="off" disabled={!workflow.canEdit} />
                {/snippet}
            </Field>
            {#if draft.kind === "login"}
                {@const loginDraft = draft as import("../secret-draft").LoginDraft}
                <Field id="username" label="Username">
                    {#snippet children(control)}
                        <input {...control} bind:value={loginDraft.username} autocomplete="off" disabled={!workflow.canEdit} />
                    {/snippet}
                </Field>
                <Field id="secret" label="Password">
                    {#snippet children(control)}
                        <div class="flex gap-2">
                            <PasswordInput {...control} bind:value={loginDraft.secret} autocomplete="new-password" disabled={!workflow.canEdit} />
                            <GeneratorDialog onuse={(value) => { if (editor && editor.draft.kind === "login") { editor.draft.secret = value; editor.confirmPassword = value; } }} />
                        </div>
                    {/snippet}
                </Field>
                {#if loginDraft.secret}
                    <Field id="confirm-secret" label="Confirm password" required>
                        {#snippet children(control)}
                            <PasswordInput {...control} bind:value={editor.confirmPassword} autocomplete="new-password" disabled={!workflow.canEdit} />
                        {/snippet}
                    </Field>
                {/if}
                <Field id="totp" label="Authenticator key (TOTP)" description="Store a Base32 key or otpauth TOTP URI. View local codes in item details.">
                    {#snippet children(control)}
                        <PasswordInput {...control} bind:value={draft.totp} autocomplete="off" disabled={!workflow.canEdit} />
                    {/snippet}
                </Field>
                <fieldset class="flex min-w-0 flex-col gap-3" disabled={!workflow.canEdit}>
                    <legend class="text-sm font-medium">Websites</legend>
                    {#each draft.uris as uri, i (uri)}
                        <div class="flex flex-wrap items-end gap-2">
                            <Field id={`uri-${i}`} label={`Website ${i + 1}`}>
                                {#snippet children(control)}
                                    <input {...control} bind:value={uri.uri} placeholder="https://example.com" />
                                {/snippet}
                            </Field>
                            <Field id={`match-${i}`} label={`Match rule ${i + 1}`}>
                                {#snippet children(control)}
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
                                <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
                            </button>
                        </div>
                    {/each}
                    <button class="btn btn-secondary btn-md self-start" type="button" onclick={() => draft.uris.push({ uri: "", match: null })}>
                        <svg class="w-4 h-4 mr-1" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></svg>Add website
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
                {@const issue = customFieldIssue(editor.record, draft.fields, i)}
                <div class="flex flex-wrap items-end gap-2">
                    <Field id={`field-name-${i}`} label={`Field name ${i + 1}`} required>
                        {#snippet children(control)}
                            <input {...control} bind:value={field.name} />
                        {/snippet}
                    </Field>
                    <Field id={`field-type-${i}`} label={`Field type ${i + 1}`}>
                        {#snippet children(control)}
                            <select {...control} bind:value={field.type}>
                                <option value="text">Text</option>
                                <option value="hidden">Hidden</option>
                                <option value="boolean">Boolean</option>
                            </select>
                        {/snippet}
                    </Field>
                    <Field id={`field-value-${i}`} label={`Field value ${i + 1}`}>
                        {#snippet children(control)}
                            {#if field.type === "boolean"}
                                <select {...control} bind:value={field.value}>
                                    {#if field.value !== "true" && field.value !== "false"}
                                        <option value={field.value} disabled>{field.value}</option>
                                    {/if}
                                    <option value="true">True</option>
                                    <option value="false">False</option>
                                </select>
                            {:else if field.type === "hidden"}
                                <PasswordInput {...control} bind:value={field.value} autocomplete="off" />
                            {:else}
                                <input {...control} bind:value={field.value} type="text" autocomplete="off" />
                            {/if}
                        {/snippet}
                    </Field>
                    <button class="btn btn-ghost btn-md text-danger-text" type="button" aria-label={`Remove custom field ${i + 1}`} onclick={() => draft.fields.splice(i, 1)}>
                        <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
                    </button>
                </div>
            {/each}
            <button class="btn btn-secondary btn-md self-start" type="button" onclick={() => draft.fields.push({ name: "", value: "", type: "text" })}>
                <svg class="w-4 h-4 mr-1" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></svg>Add custom field
            </button>
        </fieldset>
        <Field id="notes" label="Notes">
            {#snippet children(control)}
                <textarea {...control} bind:value={draft.notes} rows="3" disabled={!workflow.canEdit}></textarea>
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
                {workflow.busy ? "Saving..." : (editor.record ? "Save changes" : "Create secret")}
            </button>
            <button class="btn btn-secondary btn-md" type="button" disabled={!workflow.canEdit} onclick={handleCancel}>
                Cancel
            </button>
        </div>
        </form>
    </Card>
{:else}
    <div class="flex items-center justify-center h-64">
        <p class="text-muted-foreground">No editor available</p>
    </div>
{/if}
