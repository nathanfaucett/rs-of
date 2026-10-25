import { OidcClient } from "@aicacia/oidc-client";
import { isTauri } from "@tauri-apps/api/core";
import { env } from "$env/dynamic/public";
import icon256x256Png from "$lib/assets/icon256x256.png";
import { getIdpApiUrl } from "./idpClient.svelte";
import { observeOnboarding, type OnboardingObservation } from "./onboarding";

let storedClient: OidcClient | undefined;
let webLogin = $state.raw<{ base: string; clientId: string }>();
const CLIENT_ID = $derived(
    webLogin && webLogin.base === getIdpApiUrl()?.replace(/\/$/, "")
        ? webLogin.clientId
        : undefined,
);

export async function ensureOidcClientId(
    observation?: OnboardingObservation,
): Promise<string> {
    webLogin = undefined;
    const snapshot = observation ?? (await observeOnboarding());
    const base = getIdpApiUrl()?.replace(/\/$/, "");
    if (snapshot.state !== "ready" || snapshot.target.idp !== base)
        throw new Error("Verified public login Client is unavailable");
    webLogin = { base, clientId: snapshot.loginClient.client_id };
    return webLogin.clientId;
}

const oidcClient = $derived.by(
    () =>
        new OidcClient({
            projectOwned: true,
            clientConfig: {
                authority: getIdpApiUrl(),
                redirectUri: `${env.PUBLIC_URL}/callback`,
                clientId: CLIENT_ID,
                responseType: "code",
                registration: {
                    application: {
                        name: "IdP",
                        uri: "local.aicacia.com",
                    },
                    clientId: CLIENT_ID,
                    clientName: "IdP",
                    scope: "openid profile address offline email phone",
                    redirectUris: [`${env.PUBLIC_URL}/callback`],
                    postLogoutRedirectUris: [`${env.PUBLIC_URL}/logout`],
                    logoUri: `${env.PUBLIC_URL}${icon256x256Png}`,
                    clientUri: `${env.PUBLIC_URL}`,
                    policyUri: `${env.PUBLIC_URL}/policy`,
                    tosUri: `${env.PUBLIC_URL}/terms`,
                    profile: "web_application",
                    clientType: "public",
                    tokenEndpointAuthMethod: "none",
                    grantTypes: ["authorization_code", "refresh_token"],
                    responseTypes: ["code"],
                    accessTokenExpiry: 3600,
                    refreshTokenExpiry: 604800,
                },
            },
            fetch,
            disableNativeRequests: true,
            storage: isTauri() ? "memory" : "local",
        }),
);

export function getOidcClient() {
    storedClient = oidcClient;
    return storedClient;
}

export function clearOidcSession(): void {
    storedClient?.clearStoredTokenResponse();
    webLogin = undefined;
    if (isTauri()) {
        if (typeof window === "undefined") return;
        const storage = window.localStorage;
        for (let index = storage.length - 1; index >= 0; index--) {
            const key = storage.key(index);
            if (
                key?.startsWith("oidc.token:") ||
                key?.startsWith("oidc.pkce:")
            ) {
                storage.removeItem(key);
            }
        }
    } else if (!storedClient && typeof window !== "undefined") {
        const authority = getIdpApiUrl()?.replace(/\/$/, "");
        if (authority) {
            window.localStorage.removeItem(`oidc.token:${authority}:response`);
        }
    }
}
