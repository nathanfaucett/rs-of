import {
  type AuthorizationRequest,
  type ClientRegistration,
  ResponseError,
} from "@ofsdk/idp-client";

export type PendingAuthorizationRequest = Omit<AuthorizationRequest, "clientId"> & {
  clientId?: string | undefined;
};

function string(value: unknown): string | undefined {
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

function strings(value: unknown): string[] | undefined {
  if (value == null) return undefined;
  if (!Array.isArray(value) || !value.every((entry) => typeof entry === "string")) {
    throw new Error(
      "Registration lists must contain only strings. Restart sign-in from the application."
    );
  }
  return value;
}

export function normalizeRegistration(
  value: string | undefined,
  applicationId: string | undefined,
  request: PendingAuthorizationRequest
): ClientRegistration {
  if (!value || !applicationId)
    throw new Error(
      "New clients require application_id and registration metadata. Restart sign-in from the application."
    );
  let record: Record<string, unknown>;
  try {
    record = JSON.parse(value);
    if (!record || typeof record !== "object" || Array.isArray(record)) throw new Error();
  } catch {
    throw new Error("Invalid registration metadata. Restart sign-in from the application.");
  }
  if ("clientId" in record || "client_id" in record)
    throw new Error(
      "Registration must not supply a client ID. Restart sign-in from the application."
    );
  const {
    application: applicationValue,
    clientName: camelClientName,
    client_name: snakeClientName,
    profile: profileValue,
    tokenEndpointAuthMethod: camelTokenEndpointAuthMethod,
    token_endpoint_auth_method: snakeTokenEndpointAuthMethod,
    redirectUris,
    redirect_uris,
    allowedGrantTypes,
    allowed_grant_types,
    grantTypes,
    grant_types,
    responseTypes,
    response_types,
    allowedScopes,
    allowed_scopes,
    scope,
    clientType,
    client_type,
    clientUri,
    client_uri,
    logoUri,
    logo_uri,
    policyUri,
    policy_uri,
    tosUri,
    tos_uri,
    contacts,
  } = record;
  const application = applicationValue as Record<string, unknown> | undefined;
  const {
    uri: applicationUri,
    name: applicationName,
    description: applicationDescription,
  } = application ?? {};
  if (!application || string(applicationUri) !== applicationId)
    throw new Error(
      "application_id must match registration.application.uri. Restart sign-in from the application."
    );
  const clientName = string(camelClientName ?? snakeClientName);
  const profile = string(profileValue) as ClientRegistration["profile"] | undefined;
  const tokenEndpointAuthMethod = string(
    camelTokenEndpointAuthMethod ?? snakeTokenEndpointAuthMethod
  ) as ClientRegistration["tokenEndpointAuthMethod"] | undefined;
  if (!clientName || !profile || !tokenEndpointAuthMethod)
    throw new Error(
      "Registration is missing its client name, profile or token authentication method. Restart sign-in from the application."
    );
  return {
    application: {
      uri: applicationId,
      name: string(applicationName),
      description: string(applicationDescription),
    },
    clientName,
    profile,
    tokenEndpointAuthMethod,
    redirectUris:
      strings(redirectUris ?? redirect_uris) ?? (request.redirectUri ? [request.redirectUri] : []),
    allowedGrantTypes: (strings(
      allowedGrantTypes ?? allowed_grant_types ?? grantTypes ?? grant_types
    ) ?? ["authorization_code"]) as ClientRegistration["allowedGrantTypes"],
    responseTypes: (strings(responseTypes ?? response_types) ?? [
      request.responseType,
    ]) as ClientRegistration["responseTypes"],
    allowedScopes:
      strings(allowedScopes ?? allowed_scopes) ??
      (string(scope) ?? request.scope ?? "").split(/\s+/).filter(Boolean),
    clientType: string(clientType ?? client_type) as ClientRegistration["clientType"],
    clientUri: string(clientUri ?? client_uri),
    logoUri: string(logoUri ?? logo_uri),
    policyUri: string(policyUri ?? policy_uri),
    tosUri: string(tosUri ?? tos_uri),
    contacts: strings(contacts),
  };
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
