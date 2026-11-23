<script lang="ts" module>
    import * as v from "valibot";
    import {
        getDeviceApiUrl,
        setDeviceApiUrl,
        validateDeviceApiUrl,
    } from "$lib/common/state/deviceClient.svelte";
    import { validateIdpApiUrl } from "$lib/common/state/idpClient.svelte";
    import { validateManagementApiUrl } from "$lib/common/state/managementClient.svelte";
    import {
        discoverServices,
        type ServiceDiscovery,
        serviceOwners,
    } from "$lib/common/state/serviceApi";
    import {
        getStorageApiUrl,
        setStorageApiUrl,
        validateStorageApiUrl,
    } from "$lib/common/state/storageApiUrl.svelte";
    import { m } from "$lib/paraglide/messages";

    const configSchema = v.objectAsync({
        storageApiUrl: v.pipeAsync(
            v.string(),
            v.nonEmpty(m.errors_message_required()),
            v.url(m.errors_message_invalid_url()),
            v.checkAsync(
                validateStorageApiUrl,
                "Storage /version must identify storage-server",
            ),
        ),
        deviceApiUrl: v.pipeAsync(
            v.string(),
            v.nonEmpty(m.errors_message_required()),
            v.url(m.errors_message_invalid_url()),
            v.checkAsync(
                validateDeviceApiUrl,
                "Device /version must identify device-server",
            ),
        ),
        idpApiUrl: v.pipeAsync(
            v.string(),
            v.nonEmpty(m.errors_message_required()),
            v.url(m.errors_message_invalid_url()),
            v.checkAsync(
                validateIdpApiUrl,
                "IdP /version must identify idp-server",
            ),
        ),
        managementApiUrl: v.pipeAsync(
            v.string(),
            v.nonEmpty(m.errors_message_required()),
            v.url(m.errors_message_invalid_url()),
            v.checkAsync(
                validateManagementApiUrl,
                "Management /version must identify management-server",
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
    import { Alert, Field } from "$lib/ui";

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
    const serviceFields = {
        idp: form.fields.idpApiUrl,
        management: form.fields.managementApiUrl,
        device: form.fields.deviceApiUrl,
        storage: form.fields.storageApiUrl,
    };
    let serviceBase = $state(
        getIdpApiUrl()?.replace(/\/idp\/$|\/idp$/, "") ?? "",
    );
    let discovery = $state.raw<ServiceDiscovery>();
    let discoveryError = $state("");
    let discovering = $state(false);

    function clearDiscovery() {
        discovery = undefined;
        discoveryError = "";
    }

    async function detect() {
        if (discovering) return;
        clearDiscovery();
        discovering = true;
        try {
            const detected = await discoverServices(serviceBase);
            if (!detected) {
                discoveryError =
                    "No supported /version endpoint was found at this base or its /idp, /management, /device, and /storage prefixes.";
                return;
            }
            discovery = detected;
            if (detected.kind === "unified") {
                for (const owner of serviceOwners)
                    serviceFields[owner].value = detected.urls[owner];
            } else {
                serviceFields[detected.owner].value = detected.url;
            }
        } finally {
            discovering = false;
        }
    }

    export function reset() {
        form.fields.storageApiUrl.value = getStorageApiUrl() ?? "";
        form.fields.deviceApiUrl.value = getDeviceApiUrl() ?? "";
        form.fields.idpApiUrl.value = getIdpApiUrl() ?? "";
        form.fields.managementApiUrl.value = getManagementApiUrl() ?? "";
        serviceBase = getIdpApiUrl()?.replace(/\/idp\/$|\/idp$/, "") ?? "";
        clearDiscovery();
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
    <Field
        label="Service base URL"
        description="Detect a unified deployment or one separate service through public /version endpoints."
    >
        {#snippet children({
            id,
            "aria-describedby": describedby,
            "aria-invalid": invalid,
        })}
            <input
                {id}
                aria-describedby={describedby}
                aria-invalid={invalid}
                type="url"
                aria-label="Service base URL"
                placeholder="https://unified.localhost:1355"
                bind:value={serviceBase}
                oninput={clearDiscovery}
            />
        {/snippet}
    </Field>
    <button
        class="btn btn-secondary btn-md"
        type="button"
        onclick={detect}
        disabled={discovering || !serviceBase.trim()}
    >
        {discovering ? "Checking services…" : "Detect services"}
    </button>
    {#if discovery}
        <Alert tone="success">
            {discovery.kind === "unified"
                ? "Unified services detected. Review the four URLs and save."
                : `Separate ${discovery.owner} service detected. Its URL is filled in; check the other services separately.`}
        </Alert>
    {/if}
    {#if discoveryError}<Alert tone="danger">{discoveryError}</Alert>{/if}
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
                oninput={clearDiscovery}
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
                oninput={clearDiscovery}
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
                oninput={clearDiscovery}
            />
        {/snippet}
    </Field>
    <Field label="Storage API URL" issues={form.fields.storageApiUrl.issues}>
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
                aria-label="Storage API URL"
                placeholder="https://node.example/storage"
                bind:value={form.fields.storageApiUrl.value}
                oninput={clearDiscovery}
            />
        {/snippet}
    </Field>
    <button class="btn btn-primary btn-md" type="submit">
        {m.env_config_save()}
    </button>
</form>
