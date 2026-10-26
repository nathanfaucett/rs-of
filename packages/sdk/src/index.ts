import {
    OidcClient,
    type OidcClientConfig,
    OidcClientError,
    type OidcClientOptions,
    type RegistrationOptions,
    type SigninOptions,
} from "@aicacia/oidc-client";

import { StorageClient } from "@aicacia/storage-client";

export type SdkOptions = {
    idpBaseUrl: string;
    storageBaseUrl: string;
    client: Omit<OidcClientConfig, "authority" | "registration"> & {
        clientId: string;
        registration: OidcClientConfig["registration"] & {
            application: { uri: string; name?: string };
        };
    };
    fetch?: OidcClientOptions["fetch"];
    disableNativeRequests?: boolean;
    projectOwned?: boolean;
};
export type Identity = Readonly<{
    issuer: string;
    subject: string;
    application: string;
    clientId: string;
}>;
export type Session = Readonly<{
    status:
        | "initializing"
        | "unauthenticated"
        | "authenticated"
        | "ended"
        | "disposed";
    identity?: Identity;
    generation: number;
}>;

export class ApplicationSdk {
    readonly #oidc: OidcClient;
    readonly #options: SdkOptions;
    #session: Session = Object.freeze({
        status: "initializing",
        generation: 0,
    });
    #listeners = new Set<(session: Session) => void>();
    #epoch = 0;
    #restore: Promise<void> | undefined;
    constructor(options: SdkOptions) {
        this.#options = structuredClone({ ...options, fetch: undefined });
        if (
            !options.client.clientId ||
            !options.client.registration?.application?.uri
        )
            throw new Error("Application URI and OAuth Client ID are required");
        StorageClient.validateServiceBase(options.storageBaseUrl);
        this.#oidc = new OidcClient({
            projectOwned: options.projectOwned ?? true,
            clientConfig: {
                ...this.#options.client,
                authority: options.idpBaseUrl,
            },
            storage: "local",
            fetch: options.fetch,
            disableNativeRequests: options.disableNativeRequests,
        });
    }
    get session(): Session {
        return this.#session;
    }
    subscribe(listener: (session: Session) => void): () => void {
        if (this.#session.status === "disposed") return () => {};
        this.#listeners.add(listener);
        listener(this.#session);
        return () => {
            this.#listeners.delete(listener);
        };
    }
    #publish(status: Session["status"], identity?: Identity) {
        this.#session = Object.freeze({
            status,
            identity,
            generation: this.#epoch,
        });
        for (const listener of this.#listeners) listener(this.#session);
    }
    #active(epoch = this.#epoch) {
        if (
            epoch !== this.#epoch ||
            this.#session.status === "disposed" ||
            this.#session.status === "ended"
        )
            throw new Error("Authentication context has ended");
    }
    #terminal(error: unknown): boolean {
        if (error instanceof OidcClientError)
            return (
                [
                    "NO_ACCESS_TOKEN",
                    "NO_REFRESH_TOKEN",
                    "INVALID_USERINFO_RESPONSE",
                ].includes(error.code) || error.details?.status === 401
            );
        const response = (error as { response?: Response } | null)?.response;
        return response?.status === 401 || response?.status === 400;
    }
    async #identity(epoch: number) {
        try {
            const verified = await this.#oidc.getVerifiedIdentity();
            this.#active(epoch);
            const identity = Object.freeze({
                issuer: verified.issuer,
                subject: verified.subject,
                application: this.#options.client.registration.application.uri,
                clientId: this.#options.client.clientId,
            });
            const old = this.#session.identity;
            if (
                old &&
                (old.subject !== identity.subject ||
                    old.issuer !== identity.issuer)
            ) {
                this.#end("ended");
                throw new Error("Identity changed; sign in again");
            }
            if (!old) this.#publish("authenticated", identity);
        } catch (error) {
            if (epoch === this.#epoch && this.#terminal(error))
                this.#end("ended");
            throw new Error("Unable to establish authenticated identity");
        }
    }
    restore(): Promise<void> {
        this.#active();
        if (!this.#restore) {
            const epoch = this.#epoch;
            this.#restore = (async () => {
                if (!this.#oidc.getStoredTokenResponse()) {
                    if (this.#session.status === "authenticated")
                        this.#end("ended");
                    else this.#publish("unauthenticated");
                    return;
                }
                await this.#identity(epoch);
            })().finally(() => {
                this.#restore = undefined;
            });
        }
        return this.#restore;
    }
    async handleSigninCallback(url: URL): Promise<void> {
        this.#active();
        const epoch = ++this.#epoch;
        this.#publish("initializing");
        try {
            if (!(await this.#oidc.handleSigninCallback(url)))
                throw new Error("Missing authorization code");
            this.#active(epoch);
            await this.#identity(epoch);
        } catch {
            if (
                epoch === this.#epoch &&
                this.#session.status !== "authenticated"
            ) {
                this.#oidc.clearStoredTokenResponse();
                this.#publish("unauthenticated");
            }
            throw new Error("Sign-in callback could not be completed");
        }
    }
    async getAuthorizationUrl(options?: SigninOptions): Promise<URL> {
        this.#active();
        const epoch = this.#epoch;
        try {
            const url = await this.#oidc.getAuthorizationUrl(options);
            this.#active(epoch);
            return url;
        } catch {
            throw new Error("Unable to start sign-in");
        }
    }
    async signin(options?: SigninOptions): Promise<void> {
        this.#active();
        try {
            await this.#oidc.signin(options);
        } catch {
            throw new Error("Unable to start sign-in");
        }
    }
    async startRegistration(options?: RegistrationOptions): Promise<void> {
        this.#active();
        try {
            await this.#oidc.startRegistration(options);
        } catch {
            throw new Error("Unable to start registration");
        }
    }
    // Transitional app Storage composition captures this provider, never a mutable global client.
    tokenProvider(): () => Promise<string> {
        this.#active();
        if (this.#session.status !== "authenticated")
            throw new Error("Sign in before using Storage");
        const epoch = this.#epoch;
        return async () => {
            this.#active(epoch);
            await this.#identity(epoch);
            this.#active(epoch);
            const token = await this.#oidc.getAccessToken();
            this.#active(epoch);
            return token;
        };
    }
    #end(status: "ended" | "disposed") {
        ++this.#epoch;
        try {
            this.#oidc.clearStoredTokenResponse();
        } catch {
            throw new Error(
                "Unable to remove persisted credentials; clear browser site data",
            );
        } finally {
            this.#publish(status);
        }
    }
    async signout(
        options: { provider?: boolean; redirectUri?: string } = {},
    ): Promise<{ providerLogout: "not-requested" | "initiated" | "failed" }> {
        if (this.#session.status === "disposed")
            return { providerLogout: "not-requested" };
        this.#end("ended");
        if (!options.provider) return { providerLogout: "not-requested" };
        try {
            await this.#oidc.signout({ redirectUri: options.redirectUri });
            return { providerLogout: "initiated" };
        } catch {
            return { providerLogout: "failed" };
        }
    }
    dispose(): void {
        if (this.#session.status === "disposed") return;
        try {
            this.#end("disposed");
        } finally {
            this.#listeners.clear();
        }
    }
}
