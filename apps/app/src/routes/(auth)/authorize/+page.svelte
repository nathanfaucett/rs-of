<script lang="ts">
    import type {
        AuthorizationRequest,
        CodeChallengeMethod,
        ClientRegistration,
    } from "@aicacia/idp-client";
    import { page } from "$app/state";
    import { m } from "$lib/paraglide/messages";
    import { Card, Page, PageHeader } from "$lib/ui";
    import Authorize from "./_Authorize.svelte";
    import {
        normalizeRegistration,
        type PendingAuthorizationRequest,
    } from "./_onboarding";

    let { data } = $props();
    let request = $derived.by(() => {
        const params = page.url.searchParams;
        const errors: string[] = [];
        const responseType = params.get("response_type");
        const responseMode = params.get("response_mode");
        const redirectUri = params.get("redirect_uri");
        const scope = params.get("scope");
        if (!responseType) errors.push(m.authorize_response_type_required());
        else if (responseType !== "code")
            errors.push(
                m.authorize_response_type_invalid({
                    value: responseType,
                    allowed: "code",
                }),
            );
        if (!responseMode) errors.push(m.authorize_response_mode_required());
        else if (responseMode !== "query")
            errors.push(
                m.authorize_response_mode_invalid({
                    value: responseMode,
                    allowed: "query",
                }),
            );
        if (!redirectUri) errors.push(m.authorize_redirect_uri_required());
        if (!scope) errors.push(m.authorize_scope_required());
        const applicationId = params.get("application_id") ?? undefined;
        const authorizationRequest: PendingAuthorizationRequest = {
            clientId: params.get("client_id") || undefined,
            responseType: responseType as AuthorizationRequest["responseType"],
            responseMode: responseMode as AuthorizationRequest["responseMode"],
            redirectUri,
            scope,
            state: params.get("state"),
            nonce: params.get("nonce"),
            codeChallenge: params.get("code_challenge"),
            codeChallengeMethod:
                (params.get("code_challenge_method") as CodeChallengeMethod) ??
                undefined,
        };
        let registration: ClientRegistration | undefined;
        // Existing clients use only server metadata, never URL-supplied registration.
        if (!authorizationRequest.clientId) {
            try {
                registration = normalizeRegistration(
                    params.get("registration") ?? undefined,
                    applicationId,
                    authorizationRequest,
                );
            } catch (cause) {
                errors.push(
                    cause instanceof Error
                        ? cause.message
                        : "Invalid registration metadata.",
                );
            }
        }
        return { errors, authorizationRequest, applicationId, registration };
    });
</script>

<Page width="narrow">
    <PageHeader title="Authorize" />
    <Card>
        {#if request.errors.length === 0}
            {#key request}
                <Authorize
                    userInfo={data.userInfo}
                    authorizationRequest={request.authorizationRequest}
                    applicationId={request.applicationId}
                    registration={request.registration}
                />
            {/key}
        {:else}
            <section role="alert">
                <h2>{m.authorize_invalid_request()}</h2>
                <ul class="list-inside list-disc space-y-1 text-sm">
                    {#each request.errors as error (error)}
                        <li>{error}</li>
                    {/each}
                </ul>
                <p>
                    Return to the application and restart sign-in with a valid
                    authorization request.
                </p>
            </section>
        {/if}
    </Card>
</Page>
