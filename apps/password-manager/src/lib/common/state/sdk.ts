import { ApplicationSdk } from "@of/sdk";
import { env } from "$env/dynamic/public";
import icon256x256Png from "$lib/assets/icon256x256.png";

let authority = env.PUBLIC_IDP_BASE_URL ?? "";
try {
    const preference: unknown = JSON.parse(
        localStorage.getItem("idp-api-url") ?? "null",
    );
    if (typeof preference === "string") authority = preference;
} catch {}
let sdk: ApplicationSdk | undefined;

function createSdk(idpBaseUrl: string): ApplicationSdk {
    return new ApplicationSdk({
        idpBaseUrl,
        storageBaseUrl: env.PUBLIC_STORAGE_BASE_URL ?? "",
        disableNativeRequests: true,
        client: {
            redirectUri: `${env.PUBLIC_URL}/callback`,
            responseType: "code",
            registration: {
                application: {
                    name: "Aicacia Password Manager",
                    uri: "password-manager.aicacia.com",
                },
                clientName: "Password Manager",
                scope: "openid profile address offline email phone storage",
                redirectUris: [`${env.PUBLIC_URL}/callback`],
                postLogoutRedirectUris: [`${env.PUBLIC_URL}/signin`],
                logoUri: `${env.PUBLIC_URL}${icon256x256Png}`,
                clientUri: `${env.PUBLIC_URL}`,
                policyUri: `${env.PUBLIC_URL}/policy`,
                tosUri: `${env.PUBLIC_URL}/terms`,
                profile: "web_application",
                clientType: "public",
                tokenEndpointAuthMethod: "none",
                grantTypes: [
                    "authorization_code",
                    "refresh_token",
                    "urn:ietf:params:oauth:grant-type:token-exchange",
                ],
                responseTypes: ["code"],
                accessTokenExpiry: 3600,
                refreshTokenExpiry: 604800,
            },
        },
    });
}

export function getIdpApiUrl(): string {
    return authority;
}
export function getSdk(): ApplicationSdk {
    if (!sdk) sdk = createSdk(authority);
    return sdk;
}
export function setIdpApiUrl(value: string): void {
    const url = new URL(value);
    if (
        !["http:", "https:"].includes(url.protocol) ||
        url.username ||
        url.password ||
        url.search ||
        url.hash
    )
        throw new Error(
            "IdP API URL must use HTTP or HTTPS without credentials, query, or fragment",
        );
    const next = url.href.replace(/\/$/, "");
    if (
        next === authority &&
        sdk &&
        !["ended", "disposed"].includes(sdk.session.status)
    )
        return;
    const replacement = createSdk(next);
    sdk?.dispose();
    localStorage.setItem("idp-api-url", JSON.stringify(next));
    authority = next;
    sdk = replacement;
}
