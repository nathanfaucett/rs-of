<script lang="ts" module>
    export interface AuthorizeProps {
        userInfo: OidcUserInfo;
        authorizationRequest: PendingAuthorizationRequest;
        applicationId?: string;
        registration?: ClientRegistration;
    }
</script>

<script lang="ts">
    import type {
        ClientConsentMetadata,
        ClientRegistration,
    } from "@aicacia/idp-client";
    import type { OidcUserInfo } from "@aicacia/oidc-client";
    import LoaderCircle from "@lucide/svelte/icons/loader-circle";
    import { isTauri } from "@tauri-apps/api/core";
    import { onMount } from "svelte";
    import { goto } from "$app/navigation";
    import { resolve } from "$app/paths";
    import { idpApi } from "$lib/common/state/idpClient.svelte";
    import { m } from "$lib/paraglide/messages";
    import AuthorizeClient from "./_AuthorizeClient.svelte";
    import {
        authorizationError,
        loadConsent,
        registerClient,
        type PendingAuthorizationRequest,
    } from "./_onboarding";
    import {
        rejectAuthorizationRequest,
        resolveAuthorizationRequest,
    } from "./_utils";

    let {
        userInfo,
        authorizationRequest,
        applicationId,
        registration,
    }: AuthorizeProps = $props();
    let client = $state.raw<ClientConsentMetadata>();
    let loading = $state(true);
    let error = $state<string>();

    onMount(() => {
        let active = true;
        async function initialize() {
            try {
                if (authorizationRequest.clientId) {
                    const metadata = await loadConsent(
                        authorizationRequest.clientId,
                        applicationId,
                    );
                    if (!active) return;
                    client = metadata;
                    const response = await idpApi.isAllowedForUser({
                        isAllowedForUserRequest: {
                            clientId: metadata.clientId,
                            redirectUri: authorizationRequest.redirectUri ?? "",
                            scope: authorizationRequest.scope ?? "",
                        },
                    });
                    if (active && response.allowed)
                        await authorize(metadata.clientId);
                } else if (!registration) {
                    throw new Error(
                        "No client ID or valid registration was supplied. Restart sign-in from the application.",
                    );
                }
            } catch (cause) {
                if (active) error = authorizationError(cause);
            } finally {
                if (active) loading = false;
            }
        }
        void initialize();
        return () => {
            active = false;
        };
    });

    async function authorize(clientId: string) {
        await resolveAuthorizationRequest({
            ...authorizationRequest,
            clientId,
        });
        if (isTauri()) await goto(resolve("/"));
    }

    async function onAllow() {
        if (!client) return;
        loading = true;
        error = undefined;
        try {
            await idpApi.approveForUser({
                approveForUserRequest: {
                    clientId: client.clientId,
                    redirectUri: authorizationRequest.redirectUri ?? "",
                    scope: authorizationRequest.scope ?? "",
                },
            });
            await authorize(client.clientId);
        } catch (cause) {
            error = authorizationError(cause);
        } finally {
            loading = false;
        }
    }

    async function onDeny() {
        loading = true;
        try {
            await rejectAuthorizationRequest(
                authorizationRequest,
                "access_denied",
                m.authorize_access_denied_reason(),
            );
        } catch (cause) {
            error = authorizationError(cause);
        } finally {
            loading = false;
        }
    }

    async function onRegister() {
        if (!registration || !applicationId) return;
        loading = true;
        error = undefined;
        try {
            client = await registerClient(registration, applicationId);
        } catch (cause) {
            error = authorizationError(cause);
        } finally {
            loading = false;
        }
    }
</script>

{#snippet registrationForm(request: ClientRegistration)}
    <section>
        <h2>{m.authorize_new_client_request()}</h2>
        <p>
            This application is requesting a new client registration. These
            details were supplied by the application and are not yet verified by
            the server.
        </p>
        <dl class="mt-3 space-y-1 text-sm">
            <div>
                <dt class="font-semibold">Application</dt>
                <dd>{request.application.uri}</dd>
            </div>
            <div>
                <dt class="font-semibold">Client</dt>
                <dd>{request.clientName}</dd>
            </div>
            <div>
                <dt class="font-semibold">Redirect URIs</dt>
                <dd>{request.redirectUris?.join(", ")}</dd>
            </div>
            <div>
                <dt class="font-semibold">Registration scopes</dt>
                <dd>{request.allowedScopes?.join(", ") || "-"}</dd>
            </div>
            <div>
                <dt class="font-semibold">Requested scopes</dt>
                <dd>{authorizationRequest.scope || "-"}</dd>
            </div>
        </dl>
        <p>
            Accept registers the client. You will review server-approved details
            before granting access.
        </p>
        <div class="mt-4 flex flex-row justify-center gap-4">
            <button
                class="btn btn-primary btn-md"
                type="button"
                disabled={loading}
                onclick={onRegister}>{m.client_accept()}</button
            >
            <button
                class="btn btn-secondary btn-md"
                type="button"
                disabled={loading}
                onclick={onDeny}>{m.client_reject()}</button
            >
        </div>
    </section>
{/snippet}

{#if error && !registration}
    <section role="alert">
        <h2>{m.authorize_invalid_request()}</h2>
        <p>{error}</p>
        <button
            class="btn btn-secondary btn-md mt-4"
            type="button"
            disabled={loading}
            onclick={onDeny}
        >
            {m.authorize_button_deny()}
        </button>
    </section>
{:else if error && registration}
    <section role="alert">
        <h2>{m.authorize_invalid_request()}</h2>
        <p>{error}</p>
    </section>
    {@render registrationForm(registration)}
{:else if loading}
    <div
        class="flex flex-row items-center justify-center py-8"
        role="status"
        aria-label="Loading authorization"
    >
        <LoaderCircle class="animate-spin" aria-hidden="true" />
    </div>
{:else if client}
    <AuthorizeClient
        {userInfo}
        {client}
        disabled={loading}
        {onAllow}
        {onDeny}
    />
{:else if registration}
    {@render registrationForm(registration)}
{/if}
