import {
    type AuthorizationRequest,
    type ClientConsentMetadata,
    type ClientRegistration,
    ResponseError,
} from "@aicacia/idp-client";
import { replaceState } from "$app/navigation";
import { page } from "$app/state";
import { idpApi } from "$lib/common/state/idpClient.svelte";

export type PendingAuthorizationRequest = Omit<
    AuthorizationRequest,
    "clientId"
> & { clientId?: string };

function string(value: unknown): string | undefined {
    return typeof value === "string" && value.length > 0 ? value : undefined;
}

function strings(value: unknown): string[] | undefined {
    if (value == null) return undefined;
    if (
        !Array.isArray(value) ||
        !value.every((entry) => typeof entry === "string")
    ) {
        throw new Error(
            "Registration lists must contain only strings. Restart sign-in from the application.",
        );
    }
    return value;
}

export function normalizeRegistration(
    value: string | undefined,
    applicationId: string | undefined,
    request: PendingAuthorizationRequest,
): ClientRegistration {
    if (!value || !applicationId)
        throw new Error(
            "New clients require application_id and registration metadata. Restart sign-in from the application.",
        );
    let record: Record<string, unknown>;
    try {
        record = JSON.parse(value);
        if (!record || typeof record !== "object" || Array.isArray(record))
            throw new Error();
    } catch {
        throw new Error(
            "Invalid registration metadata. Restart sign-in from the application.",
        );
    }
    if ("clientId" in record || "client_id" in record)
        throw new Error(
            "Registration must not supply a client ID. Restart sign-in from the application.",
        );
    const application = record.application as
        | Record<string, unknown>
        | undefined;
    if (!application || string(application.uri) !== applicationId)
        throw new Error(
            "application_id must match registration.application.uri. Restart sign-in from the application.",
        );
    const clientName = string(record.clientName ?? record.client_name);
    const profile = string(record.profile) as
        | ClientRegistration["profile"]
        | undefined;
    const tokenEndpointAuthMethod = string(
        record.tokenEndpointAuthMethod ?? record.token_endpoint_auth_method,
    ) as ClientRegistration["tokenEndpointAuthMethod"] | undefined;
    if (!clientName || !profile || !tokenEndpointAuthMethod)
        throw new Error(
            "Registration is missing its client name, profile or token authentication method. Restart sign-in from the application.",
        );
    return {
        application: {
            uri: applicationId,
            name: string(application.name),
            description: string(application.description),
        },
        clientName,
        profile,
        tokenEndpointAuthMethod,
        redirectUris:
            strings(record.redirectUris ?? record.redirect_uris) ??
            (request.redirectUri ? [request.redirectUri] : []),
        allowedGrantTypes: (strings(
            record.allowedGrantTypes ??
                record.allowed_grant_types ??
                record.grantTypes ??
                record.grant_types,
        ) ?? ["authorization_code"]) as ClientRegistration["allowedGrantTypes"],
        responseTypes: (strings(
            record.responseTypes ?? record.response_types,
        ) ?? [request.responseType]) as ClientRegistration["responseTypes"],
        allowedScopes:
            strings(record.allowedScopes ?? record.allowed_scopes) ??
            (string(record.scope) ?? request.scope ?? "")
                .split(/\s+/)
                .filter(Boolean),
        clientType: string(
            record.clientType ?? record.client_type,
        ) as ClientRegistration["clientType"],
        clientUri: string(record.clientUri ?? record.client_uri),
        logoUri: string(record.logoUri ?? record.logo_uri),
        policyUri: string(record.policyUri ?? record.policy_uri),
        tosUri: string(record.tosUri ?? record.tos_uri),
        contacts: strings(record.contacts),
    };
}

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

export function authorizationError(error: unknown): string {
    if (error instanceof ResponseError) {
        if (error.response.status === 403)
            return "Permission denied. Ask an administrator for idp.clients.register (and idp.applications.create for a new application) when registering, or access to this client when authorizing.";
        if (error.response.status === 404)
            return "The client ID you provided does not exist. Please register a new client to continue.";
        if (error.response.status === 401)
            return "Your session has expired. Sign in again and restart this request.";
        return `Authorization failed (HTTP ${error.response.status}). Check the request with the application administrator and retry.`;
    }
    return error instanceof Error
        ? error.message
        : "Authorization failed. Check your connection and restart sign-in from the application.";
}
