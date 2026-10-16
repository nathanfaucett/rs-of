import { OidcClient } from "@aicacia/oidc-client";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { env } from "$env/dynamic/public";
import icon256x256Png from "$lib/assets/icon256x256.png";
import { getIdpApiUrl } from "./idpClient.svelte";
import { fetchSetupStatus } from "./setupApi";

let desktopClientId = $state<string | undefined>(undefined);
let storedClient: OidcClient | undefined;
let webLogin = $state.raw<{ base: string; clientId: string }>();
const CLIENT_ID = $derived(
    isTauri()
        ? desktopClientId
        : webLogin?.base === getIdpApiUrl()?.replace(/\/$/, "")
          ? webLogin.clientId
          : undefined,
);

export async function ensureOidcClientId(): Promise<string> {
    if (!isTauri()) {
        webLogin = undefined;
        const base = getIdpApiUrl()?.replace(/\/$/, "") ?? "";
        const status = await fetchSetupStatus(base);
        if (
            base !== getIdpApiUrl()?.replace(/\/$/, "") ||
            !status.ready ||
            !status.login_client
        )
            throw new Error("Verified public login Client is unavailable");
        webLogin = { base, clientId: status.login_client.client_id };
        return webLogin.clientId;
    }
    desktopClientId ??= await invoke<string>("get_desktop_client_id");
    return desktopClientId;
}

const oidcClient = $derived.by(
    () =>
        new OidcClient({
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
        desktopClientId = undefined;
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
