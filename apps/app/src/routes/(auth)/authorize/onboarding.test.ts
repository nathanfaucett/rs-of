import { ResponseError } from "@aicacia/idp-client";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
    authorizationError,
    loadConsent,
    normalizeRegistration,
    type PendingAuthorizationRequest,
    registerClient,
} from "./_onboarding";
import { resolveAuthorizationRequest } from "./_utils";

const api = vi.hoisted(() => ({
    getClient: vi.fn(),
    getRegister: vi.fn(),
    register: vi.fn(),
    authorizeJson: vi.fn(),
    redirect: vi.fn(),
    replaceState: vi.fn(),
    page: {
        url: new URL("https://idp.test/authorize"),
        state: { existing: "history-state" },
    },
}));
vi.mock("$app/navigation", () => ({ replaceState: api.replaceState }));
vi.mock("$app/state", () => ({ page: api.page }));
vi.mock("$lib/common/state/idpClient.svelte", () => ({ idpApi: api }));
vi.mock("$lib/common/util/redirectToUrl", () => ({
    redirectToUrl: api.redirect,
}));

const applicationId = "https://application.test";
const request: PendingAuthorizationRequest = {
    responseType: "code",
    responseMode: "query",
    redirectUri: "https://application.test/callback",
    scope: "openid profile",
    state: "request-state",
    codeChallenge: "original-pkce-challenge",
    codeChallengeMethod: "S256",
};
const registration = {
    application: { uri: applicationId, name: "Application" },
    client_name: "Requested client",
    profile: "native_application",
    token_endpoint_auth_method: "none",
};
const metadata = {
    application: registration.application,
    clientId: "server-issued-id",
    clientName: "Server-approved client",
    redirectUris: ["https://application.test/callback"],
    allowedScopes: ["openid"],
    clientUri: null,
    logoUri: null,
    tosUri: null,
    policyUri: null,
};

beforeEach(() => {
    vi.resetAllMocks();
    api.page.url = new URL("https://idp.test/authorize");
    api.getClient.mockResolvedValue(metadata);
    api.register.mockResolvedValue({
        ...metadata,
        clientSecret: "never-display",
    });
    api.authorizeJson.mockResolvedValue({
        code: "authorization-code",
        state: "server-state",
        iss: "https://issuer.test",
    });
});

describe("registration trust boundary", () => {
    it("normalizes snake case metadata, preserves the logical URI, and supplies request defaults without an ID", () => {
        const normalized = normalizeRegistration(
            JSON.stringify(registration),
            applicationId,
            request,
        );
        expect(normalized).toMatchObject({
            application: registration.application,
            clientName: "Requested client",
            tokenEndpointAuthMethod: "none",
            redirectUris: [request.redirectUri],
            allowedGrantTypes: ["authorization_code"],
            responseTypes: ["code"],
            allowedScopes: ["openid", "profile"],
        });
        expect(normalized).not.toHaveProperty("clientId");
    });

    it("supports camel case metadata without overwriting explicit redirect/scope restrictions", () => {
        const normalized = normalizeRegistration(
            JSON.stringify({
                application: registration.application,
                clientName: "Camel client",
                profile: "native_application",
                tokenEndpointAuthMethod: "none",
                redirectUris: ["https://application.test/other"],
                allowedScopes: [],
            }),
            applicationId,
            request,
        );
        expect(normalized.redirectUris).toEqual([
            "https://application.test/other",
        ]);
        expect(normalized.allowedScopes).toEqual([]);
    });

    it.each(["clientId", "client_id"])("rejects caller-supplied %s", (key) => {
        expect(() =>
            normalizeRegistration(
                JSON.stringify({ ...registration, [key]: "attacker-id" }),
                applicationId,
                request,
            ),
        ).toThrow("must not supply a client ID");
    });

    it.each([undefined, "", "{", "null", "[]"])(
        "rejects absent or malformed metadata: %s",
        (value) => {
            expect(() =>
                normalizeRegistration(value, applicationId, request),
            ).toThrow();
        },
    );

    it("requires a matching application URI, not a database UUID", () => {
        expect(() =>
            normalizeRegistration(
                JSON.stringify(registration),
                undefined,
                request,
            ),
        ).toThrow("application_id");
        expect(() =>
            normalizeRegistration(
                JSON.stringify(registration),
                "01900000-0000-7000-8000-000000000001",
                request,
            ),
        ).toThrow("must match");
    });

    it("rejects incomplete metadata and mixed-type lists", () => {
        expect(() =>
            normalizeRegistration(
                JSON.stringify({ application: registration.application }),
                applicationId,
                request,
            ),
        ).toThrow("missing");
        expect(() =>
            normalizeRegistration(
                JSON.stringify({
                    ...registration,
                    redirect_uris: [request.redirectUri, 4],
                }),
                applicationId,
                request,
            ),
        ).toThrow("only strings");
    });

    it("does not forward untrusted secrets or admin fields", () => {
        const normalized = normalizeRegistration(
            JSON.stringify({
                ...registration,
                client_secret: "untrusted",
                registration_access_token: "untrusted",
                allowedAudiences: ["admin"],
            }),
            applicationId,
            request,
        );
        expect(normalized).not.toHaveProperty("clientSecret");
        expect(normalized).not.toHaveProperty("registrationAccessToken");
        expect(normalized).not.toHaveProperty("allowedAudiences");
    });
});

describe("server-issued identity and consent", () => {
    it("registers without a client ID and fetches only allowlisted consent metadata with the assigned ID", async () => {
        const normalized = normalizeRegistration(
            JSON.stringify(registration),
            applicationId,
            request,
        );
        const consent = await registerClient(normalized, applicationId);
        expect(api.register).toHaveBeenCalledExactlyOnceWith({
            clientRegistration: normalized,
        });
        expect(
            api.register.mock.calls[0][0].clientRegistration,
        ).not.toHaveProperty("clientId");
        expect(api.getClient).toHaveBeenCalledExactlyOnceWith({
            clientId: "server-issued-id",
        });
        expect(api.getRegister).not.toHaveBeenCalled();
        expect(consent).toBe(metadata);
        expect(consent).not.toHaveProperty("clientSecret");
    });

    it("replaces the current URL with the issued ID before consent, preserving all other parameters and history state", async () => {
        const original = new URL("https://idp.test/authorize#consent");
        original.search = new URLSearchParams([
            ["application_id", applicationId],
            ["state", request.state ?? ""],
            ["nonce", "request-nonce"],
            ["response_type", request.responseType],
            ["response_mode", request.responseMode ?? ""],
            ["redirect_uri", request.redirectUri ?? ""],
            ["scope", request.scope ?? ""],
            ["code_challenge", request.codeChallenge ?? ""],
            ["code_challenge_method", request.codeChallengeMethod ?? ""],
            ["extra", "first"],
            ["extra", "second"],
            ["registration", JSON.stringify(registration)],
        ]).toString();
        api.page.url = original;
        api.getClient.mockImplementation(async () => {
            expect(api.replaceState).toHaveBeenCalledTimes(1);
            return metadata;
        });

        await registerClient(
            normalizeRegistration(
                JSON.stringify(registration),
                applicationId,
                request,
            ),
            applicationId,
        );

        const replacement = api.replaceState.mock.calls[0][0] as URL;
        expect(replacement.searchParams.get("client_id")).toBe(
            metadata.clientId,
        );
        expect(replacement.searchParams.has("registration")).toBe(false);
        expect(
            [...replacement.searchParams].filter(
                ([key]) => key !== "client_id",
            ),
        ).toEqual(
            [...original.searchParams].filter(
                ([key]) => key !== "registration",
            ),
        );
        expect(replacement.origin).toBe(original.origin);
        expect(replacement.pathname).toBe(original.pathname);
        expect(replacement.hash).toBe(original.hash);
        expect(api.replaceState.mock.calls[0][1]).toBe(api.page.state);
        expect(api.page.url).toBe(original);
        expect(original.searchParams.has("client_id")).toBe(false);
        expect(original.searchParams.has("registration")).toBe(true);

        const reloadedUrl = new URL(replacement);
        const clientId = reloadedUrl.searchParams.get("client_id");
        if (!clientId)
            throw new Error("Reload must retain the issued client ID");
        await loadConsent(
            clientId,
            reloadedUrl.searchParams.get("application_id") ?? undefined,
        );
        expect(api.getClient).toHaveBeenLastCalledWith({
            clientId: metadata.clientId,
        });
        expect(api.register).toHaveBeenCalledTimes(1);
    });

    it("retains the issued ID in the URL even if fetching consent fails", async () => {
        const failure = new ResponseError(new Response(null, { status: 500 }));
        api.getClient.mockRejectedValue(failure);
        await expect(
            registerClient(
                normalizeRegistration(
                    JSON.stringify(registration),
                    applicationId,
                    request,
                ),
                applicationId,
            ),
        ).rejects.toBe(failure);
        const replacement = api.replaceState.mock.calls[0][0] as URL;
        expect(replacement.searchParams.get("client_id")).toBe(
            metadata.clientId,
        );
    });

    it("does not replace the URL when registration is denied", async () => {
        const failure = new ResponseError(new Response(null, { status: 403 }));
        api.register.mockRejectedValue(failure);
        await expect(
            registerClient(
                normalizeRegistration(
                    JSON.stringify(registration),
                    applicationId,
                    request,
                ),
                applicationId,
            ),
        ).rejects.toBe(failure);
        expect(api.replaceState).not.toHaveBeenCalled();
        expect(api.getClient).not.toHaveBeenCalled();
    });

    it("refuses a registration response without an assigned ID", async () => {
        api.register.mockResolvedValue({});
        await expect(
            registerClient(
                normalizeRegistration(
                    JSON.stringify(registration),
                    applicationId,
                    request,
                ),
                applicationId,
            ),
        ).rejects.toThrow("server-issued client ID");
        expect(api.getClient).not.toHaveBeenCalled();
        expect(api.replaceState).not.toHaveBeenCalled();
    });

    it("checks existing-client identity and application against server metadata", async () => {
        await expect(loadConsent("another-id", applicationId)).rejects.toThrow(
            "does not match",
        );
        await expect(
            loadConsent(metadata.clientId, "https://other.test"),
        ).rejects.toThrow("does not match");
        expect(api.register).not.toHaveBeenCalled();
    });

    it.each([403, 404, 500])(
        "propagates lookup HTTP %s without registering or using admin metadata",
        async (status) => {
            const failure = new ResponseError(new Response(null, { status }));
            api.getClient.mockRejectedValue(failure);
            await expect(loadConsent(metadata.clientId)).rejects.toBe(failure);
            expect(api.register).not.toHaveBeenCalled();
            expect(api.getRegister).not.toHaveBeenCalled();
        },
    );

    it("distinguishes missing clients from permission denial and other failures", () => {
        expect(
            authorizationError(
                new ResponseError(new Response(null, { status: 403 })),
            ),
        ).toContain("idp.clients.register");
        expect(
            authorizationError(
                new ResponseError(new Response(null, { status: 403 })),
            ),
        ).toContain("idp.applications.create");
        expect(
            authorizationError(
                new ResponseError(new Response(null, { status: 404 })),
            ),
        ).toContain("Client not found");
        expect(
            authorizationError(
                new ResponseError(new Response(null, { status: 500 })),
            ),
        ).toContain("HTTP 500");
    });
});

describe("authorization callback", () => {
    it("uses the issued ID, preserves backend PKCE inputs and returns client_id alongside code, state and iss", async () => {
        const consent = await registerClient(
            normalizeRegistration(
                JSON.stringify(registration),
                applicationId,
                request,
            ),
            applicationId,
        );
        const assignedRequest = { ...request, clientId: consent.clientId };
        await resolveAuthorizationRequest(assignedRequest);
        expect(api.authorizeJson).toHaveBeenCalledExactlyOnceWith({
            authorizationRequest: assignedRequest,
        });
        const url = api.redirect.mock.calls[0][0] as URL;
        expect(url.searchParams.get("client_id")).toBe("server-issued-id");
        expect(url.searchParams.get("code")).toBe("authorization-code");
        expect(url.searchParams.get("state")).toBe("server-state");
        expect(url.searchParams.get("iss")).toBe("https://issuer.test");
    });

    it("retains request state when the server omits it", async () => {
        api.authorizeJson.mockResolvedValue({ code: "code" });
        await resolveAuthorizationRequest({
            ...request,
            clientId: metadata.clientId,
        });
        expect(api.redirect.mock.calls[0][0].searchParams.get("state")).toBe(
            request.state,
        );
    });

    it.each([
        { error: "invalid_request", errorDescription: "PKCE rejected" },
        {},
    ])("does not redirect when authorization fails: %o", async (response) => {
        api.authorizeJson.mockResolvedValue(response);
        await expect(
            resolveAuthorizationRequest({
                ...request,
                clientId: metadata.clientId,
            }),
        ).rejects.toThrow();
        expect(api.redirect).not.toHaveBeenCalled();
    });
});
