<script lang="ts" module>
    import * as v from "valibot";
    import {
        getDeviceApiUrl,
        setDeviceApiUrl,
        validateDeviceApiUrl,
    } from "$lib/common/state/deviceClient.svelte";
    import { validateIdpApiUrl } from "$lib/common/state/idpClient.svelte";
    import { validateManagementApiUrl } from "$lib/common/state/managementClient.svelte";
    import { getStorageApiUrl, setStorageApiUrl, validateStorageApiUrl } from "$lib/common/state/storageApiUrl.svelte";
    import { m } from "$lib/paraglide/messages";

    const configSchema = v.objectAsync({
        storageApiUrl: v.pipeAsync(
            v.string(),
            v.nonEmpty(m.errors_message_required()),
            v.url(m.errors_message_invalid_url()),
            v.checkAsync(validateStorageApiUrl, m.errors_message_invalid_url()),
        ),
        deviceApiUrl: v.pipeAsync(
            v.string(),
            v.nonEmpty(m.errors_message_required()),
            v.url(m.errors_message_invalid_url()),
            v.checkAsync(validateDeviceApiUrl, m.errors_message_invalid_url()),
        ),
        idpApiUrl: v.pipeAsync(
            v.string(),
            v.nonEmpty(m.errors_message_required()),
            v.url(m.errors_message_invalid_url()),
            v.checkAsync(validateIdpApiUrl, m.errors_message_invalid_url()),
        ),
        managementApiUrl: v.pipeAsync(
            v.string(),
            v.nonEmpty(m.errors_message_required()),
            v.url(m.errors_message_invalid_url()),
            v.checkAsync(
                validateManagementApiUrl,
                m.errors_message_invalid_url(),
            ),
        ),
    });
</script>

<script lang="ts">
    import { createForm } from "@aicacia/svelte-forms";
    import {
        getIdpApiUrl,
        setIdpApiUrl,
    } from "$lib/common/state/idpClient.svelte";
    import {
        getManagementApiUrl,
        setManagementApiUrl,
    } from "$lib/common/state/managementClient.svelte";
    import { notifications } from "$lib/common/state/notifications.svelte";
    import { Field } from "$lib/ui";

    interface Props {
        onSaved?: () => void;
    }

    let { onSaved = () => {} }: Props = $props();

    const form = createForm(configSchema, {
        storageApiUrl: getStorageApiUrl() ?? "",
        deviceApiUrl: getDeviceApiUrl() ?? "",
        idpApiUrl: getIdpApiUrl() ?? "",
        managementApiUrl: getManagementApiUrl() ?? "",
    });

    export function reset() {
        form.fields.storageApiUrl.value = getStorageApiUrl() ?? "";
        form.fields.deviceApiUrl.value = getDeviceApiUrl() ?? "";
        form.fields.idpApiUrl.value = getIdpApiUrl() ?? "";
        form.fields.managementApiUrl.value = getManagementApiUrl() ?? "";
    }

    async function onSubmit(event: SubmitEvent) {
        event.preventDefault();

        const [_input, output, error] = await form.validate();
        if (error) {
            notifications.add(m.errors_message_invalid(), "error");
            return;
        }

        setStorageApiUrl(output.storageApiUrl);
        setIdpApiUrl(output.idpApiUrl);
        setDeviceApiUrl(output.deviceApiUrl);
        setManagementApiUrl(output.managementApiUrl);
        onSaved();
    }
</script>

<form onsubmit={onSubmit} class="flex flex-col gap-4">
    <Field label={m.env_config_idp_url()} issues={form.fields.idpApiUrl.issues}>
        {#snippet children({
            id,
            "aria-describedby": describedby,
            "aria-invalid": invalid,
            required,
        })}
            <input
                {id}
                aria-describedby={describedby}
                aria-invalid={invalid}
                {required}
                type="text"
                aria-label={m.env_config_idp_url()}
                placeholder={m.env_config_idp_url_placeholder()}
                bind:value={form.fields.idpApiUrl.value}
            />
        {/snippet}
    </Field>
    <Field
        label={m.env_config_management_url()}
        issues={form.fields.managementApiUrl.issues}
    >
        {#snippet children({
            id,
            "aria-describedby": describedby,
            "aria-invalid": invalid,
            required,
        })}
            <input
                {id}
                aria-describedby={describedby}
                aria-invalid={invalid}
                {required}
                type="text"
                aria-label={m.env_config_management_url()}
                placeholder={m.env_config_management_url_placeholder()}
                bind:value={form.fields.managementApiUrl.value}
            />
        {/snippet}
    </Field>
    <Field label="Device API URL" issues={form.fields.deviceApiUrl.issues}>
        {#snippet children({
            id,
            "aria-describedby": describedby,
            "aria-invalid": invalid,
            required,
        })}
            <input
                {id}
                aria-describedby={describedby}
                aria-invalid={invalid}
                {required}
                type="url"
                aria-label="Device API URL"
                placeholder="https://node.example/device"
                bind:value={form.fields.deviceApiUrl.value}
            />
        {/snippet}
    </Field>
    <Field label="Storage API URL" issues={form.fields.storageApiUrl.issues}>
        {#snippet children({ id, "aria-describedby": describedby, "aria-invalid": invalid, required })}
            <input {id} aria-describedby={describedby} aria-invalid={invalid} {required}
                type="url" aria-label="Storage API URL" placeholder="https://node.example/storage"
                bind:value={form.fields.storageApiUrl.value} />
        {/snippet}
    </Field>
    <button class="btn btn-primary btn-md" type="submit"
        >{m.env_config_save()}</button
    >
</form>
