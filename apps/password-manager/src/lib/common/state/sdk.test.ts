import { afterEach, beforeEach, expect, it, vi } from "vitest";

vi.mock("$env/dynamic/public", () => ({
    env: {
        PUBLIC_URL: "https://app.test",
        PUBLIC_IDP_BASE_URL: "https://default.test/idp",
        PUBLIC_STORAGE_BASE_URL: "https://files.test/storage",
    },
}));
const service = vi.fn(async (input: URL | RequestInfo) => {
    const url = String(input);
    if (url.endsWith("/.well-known/openid-configuration")) {
        const base = url.replace("/.well-known/openid-configuration", "");
        return Response.json({
            issuer: "https://installation.test",
            authorization_endpoint: `${base}/authorize`,
            token_endpoint: `${base}/oauth2/token`,
            userinfo_endpoint: `${base}/userinfo`,
        });
    }
    if (url.endsWith("/oauth2/token"))
        return Response.json({ access_token: "access", expires_in: 3600 });
    if (url.endsWith("/userinfo")) return Response.json({ sub: "alice" });
    throw new Error("Unexpected controlled request");
});
beforeEach(() => {
    vi.resetModules();
    const values = new Map<string, string>();
    vi.stubGlobal("localStorage", {
        get length() {
            return values.size;
        },
        key: (i: number) => [...values.keys()][i] ?? null,
        getItem: (key: string) => values.get(key) ?? null,
        setItem: (key: string, value: string) => values.set(key, value),
        removeItem: (key: string) => values.delete(key),
    });
    vi.stubGlobal("fetch", service);
    service.mockClear();
});
afterEach(() => vi.unstubAllGlobals());
it("persists the selected IdP before redirect, restores through the real app configuration, and retires it on reconfiguration", async () => {
    const app = await import("./sdk");
    app.setIdpApiUrl("https://selected.test/prefix/idp");
    expect(localStorage.getItem("idp-api-url")).toBe(
        '"https://selected.test/prefix/idp"',
    );
    const authorization = await app.getSdk().getAuthorizationUrl();
    expect(authorization.origin + authorization.pathname).toBe(
        "https://selected.test/prefix/idp/authorize",
    );
    expect(authorization.searchParams.get("client_id")).toBe(
        "password-manager-web",
    );
    expect(
        JSON.parse(authorization.searchParams.get("registration") ?? "null"),
    ).toMatchObject({
        application: {
            uri: "password-manager.aicacia.com",
            name: "Aicacia Password Manager",
        },
        redirect_uris: ["https://app.test/callback"],
        post_logout_redirect_uris: ["https://app.test/signin"],
        token_endpoint_auth_method: "none",
    });
    vi.resetModules();
    const returning = await import("./sdk");
    const sdk = returning.getSdk();
    await sdk.handleSigninCallback(
        new URL(
            `https://app.test/callback?code=once&state=${authorization.searchParams.get("state")}`,
        ),
    );
    expect(sdk.session).toMatchObject({
        status: "authenticated",
        identity: {
            issuer: "https://installation.test",
            subject: "alice",
            application: "password-manager.aicacia.com",
        },
    });
    const token = sdk.tokenProvider();
    returning.setIdpApiUrl("https://other.test/idp");
    expect(sdk.session.status).toBe("disposed");
    const calls = service.mock.calls.length;
    await expect(token()).rejects.toThrow("ended");
    expect(service.mock.calls).toHaveLength(calls);
    expect(returning.getSdk()).not.toBe(sdk);
});
it("rejects invalid reconfiguration without changing the active SDK or preference", async () => {
    const app = await import("./sdk");
    const sdk = app.getSdk();
    expect(() =>
        app.setIdpApiUrl("https://user:password@bad.test/idp"),
    ).toThrow();
    expect(app.getSdk()).toBe(sdk);
    expect(app.getIdpApiUrl()).toBe("https://default.test/idp");
});
