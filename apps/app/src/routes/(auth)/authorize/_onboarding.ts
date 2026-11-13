import type { ClientConsentMetadata, ClientRegistration } from "@ofsdk/idp-client";
import { replaceState } from "$app/navigation";
import { page } from "$app/state";
import { idpApi } from "$lib/common/state/idpClient.svelte";
export { normalizeRegistration, authorizationError, type PendingAuthorizationRequest } from "./registration";

export async function loadConsent(
    clientId: string,
    applicationId?: string,
): Promise<ClientConsentMetadata> {
    const client = await idpApi.getClient({ clientId });
    if (
        client.clientId !== clientId ||
        (applicationId && client.application.uri !== applicationId)
    ) {
        throw new Error(
            "Client metadata does not match this authorization request. Restart sign-in from the application.",
        );
    }
    return client;
}

export async function registerClient(
    registration: ClientRegistration,
    applicationId: string,
): Promise<ClientConsentMetadata> {
    const registered = await idpApi.register({
        clientRegistration: registration,
    });
    if (!registered.clientId)
        throw new Error(
            "Registration did not return a server-issued client ID. Contact the administrator before retrying.",
        );
    const url = new URL(page.url);
    url.searchParams.set("client_id", registered.clientId);
    url.searchParams.delete("registration");
    replaceState(url, page.state);
    return loadConsent(registered.clientId, applicationId);
}
