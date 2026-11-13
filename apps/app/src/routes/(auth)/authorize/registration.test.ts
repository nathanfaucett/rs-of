import { ResponseError } from "@ofsdk/idp-client";
import { describe, expect, it } from "vitest";
import { normalizeRegistration, authorizationError, type PendingAuthorizationRequest } from "./registration";

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
        ).toContain("does not exist");
        expect(
            authorizationError(
                new ResponseError(new Response(null, { status: 500 })),
            ),
        ).toContain("HTTP 500");
    });