import { isTauri } from "@tauri-apps/api/core";
import {
    Configuration,
    SetupApi as DefaultApi,
    type CreateResult,
    CreateResultFromJSON,
    SetupIntentToJSON,
    JSONApiResponse,
    ResponseError,
    FetchError,
} from "@aicacia/idp-client";
import {
    Configuration as IndependentConfiguration,
    IndependentSetupApi as IndependentApi,
    type IndependentSetupStatus,
    IndependentSetupStatusFromJSON,
    JSONApiResponse as IndependentJSONApiResponse,
    ResponseError as IndependentResponseError,
    FetchError as IndependentFetchError,
} from "@aicacia/idp-client";

export class SetupEvidenceError extends Error {}
export class SetupPrerequisiteError extends Error {}
export type SetupOwner = "idp" | "device" | "storage";
export type SetupStatus = {
    stage: "installation" | "unavailable" | "ready";
    state:
        | "awaiting_choice"
        | "creating"
        | "joining"
        | "unfinished"
        | "unavailable"
        | "ready";
    ready: boolean;
    owners: SetupOwner[];
    login_client?: { client_id: string; scopes: string[] };
};
export type SetupCreateIntent = {
    operation_nonce: string;
    administrator: string;
    node: string;
};
export type SetupCreateRequest = SetupCreateIntent & { password: string };
export type SetupJoinRequest = {
    operation_nonce: string;
    node: string;
    peer: string;
};
export type SetupJoinIntent = SetupJoinRequest & { issuer: string };
export type SetupIntent = SetupCreateIntent | SetupJoinIntent;
export type SetupSessionIntent =
    | SetupCreateIntent
    | (SetupJoinRequest & { issuer?: string });
export type SetupHost = { base: string; status: SetupStatus };
export type SetupBases = {
    idp: string;
    device?: string | null;
    storage?: string | null;
};

export function setupBase(base: string): string {
    let url: URL;
    try {
        url = new URL(base);
    } catch {
        throw new SetupPrerequisiteError(
            "A verified HTTPS service base is required for setup",
        );
    }
    if (
        url.protocol !== "https:" ||
        url.username ||
        url.password ||
        url.search ||
        url.hash
    ) {
        throw new SetupPrerequisiteError(
            "A verified HTTPS service base is required for setup",
        );
    }
    return url.href.replace(/\/$/, "");
}

export function requireSetupUiOrigin(origin: string): void {
    let url: URL;
    try {
        url = new URL(origin);
    } catch {
        throw new SetupPrerequisiteError(
            "Setup requires the current HTTPS UI origin",
        );
    }
    if (url.protocol !== "https:" || url.origin !== origin)
        throw new SetupPrerequisiteError(
            "Setup requires the current HTTPS UI origin",
        );
}

const owners: SetupOwner[] = ["idp", "device", "storage"];

export function requireCurrentSetupUiOrigin(): void {
    const location = typeof window === "undefined" ? undefined : window.location;
    const origin =
        location?.origin === "null" &&
        location.protocol === "tauri:" &&
        location.host === "localhost"
            ? "tauri://localhost"
            : (location?.origin ?? "");
    if (isTauri() && ["tauri://localhost", "http://tauri.localhost"].includes(origin))
        return;
    requireSetupUiOrigin(origin);
}

const setupInit: RequestInit = {
    credentials: "omit",
    redirect: "error",
    cache: "no-store",
};

function setupClient(base: string, code?: string): DefaultApi {
    const target = setupBase(base);
    if (!target.endsWith("/idp"))
        throw new Error("Unified setup requires the configured IdP base");
    if (code !== undefined) requireSetupCode(code);
    return new DefaultApi(
        new Configuration({
            basePath: target,
            fetchApi: fetch,
            headers:
                code === undefined ? {} : { Authorization: `Setup ${code}` },
        }),
    );
}

export async function fetchSetupStatus(base: string): Promise<SetupStatus> {
    const target = setupBase(base);
    let coordinated = target.endsWith("/idp");
    try {
        if (coordinated) {
            try {
                const response =
                    await setupClient(base).setupStatusRaw(setupInit);
                if (response.raw.status !== 200)
                    throw new Error("Invalid Setup status");
                return readSetupStatus(
                    await response.value(),
                    response.raw.status,
                );
            } catch (error) {
                if (
                    !(error instanceof ResponseError) ||
                    error.response.status !== 404
                )
                    throw error;
                coordinated = false;
            }
        }
        const api = new IndependentApi(
            new IndependentConfiguration({ basePath: target, fetchApi: fetch }),
        );
        const response = await api.independentSetupStatusRaw(setupInit);
        if (response.raw.status !== 200)
            throw new Error("Invalid Setup status");
        return readSetupStatus(await response.value(), response.raw.status);
    } catch (error) {
        if (
            coordinated &&
            error instanceof ResponseError &&
            error.response.status === 503
        ) {
            return readSetupStatus(
                await new JSONApiResponse(
                    error.response,
                    CreateResultFromJSON,
                ).value(),
                503,
            );
        }
        if (
            error instanceof IndependentResponseError &&
            error.response.status === 503
        ) {
            return readSetupStatus(
                await new IndependentJSONApiResponse(
                    error.response,
                    IndependentSetupStatusFromJSON,
                ).value(),
                503,
            );
        }
        if (
            error instanceof FetchError ||
            error instanceof IndependentFetchError
        )
            throw new Error(
                "Setup status request failed; verify HTTPS trust and UI origin configuration",
                { cause: error },
            );
        if (
            error instanceof ResponseError ||
            error instanceof IndependentResponseError
        ) {
            if (error.response.status >= 500)
                throw new Error("Setup status unavailable; retry observation");
            throw new SetupPrerequisiteError(
                "Check selected service configuration and status access",
            );
        }
        throw new SetupEvidenceError("Setup evidence is invalid", {
            cause: error,
        });
    }
}

export function readSetupStatus(
    status: CreateResult | IndependentSetupStatus,
    responseStatus: number,
): SetupStatus {
    if (!status || typeof status !== "object" || Array.isArray(status))
        throw new SetupEvidenceError("Setup evidence is invalid");
    const value = status;
    if (
        value.stage !== "installation" ||
        typeof value.ready !== "boolean" ||
        ![
            "awaiting_choice",
            "creating",
            "joining",
            "unfinished",
            "unavailable",
            "ready",
        ].includes(value.state as string) ||
        value.ready !== (value.state === "ready") ||
        (responseStatus === 503 && value.state !== "unavailable") ||
        !Array.isArray(value.owners) ||
        (value.owners.length !== 1 && value.owners.length !== 3) ||
        value.owners.some(
            (owner) => !owners.some((expected) => expected === owner),
        ) ||
        new Set(value.owners).size !== value.owners.length
    ) {
        throw new SetupEvidenceError("Setup evidence is invalid");
    }
    let login: SetupStatus["login_client"];
    if (
        value.ready &&
        value.owners.length === 3 &&
        value.owners.includes("idp")
    ) {
        const client = "loginClient" in value ? value.loginClient : undefined;
        if (
            responseStatus !== 200 ||
            !client ||
            typeof client.clientId !== "string" ||
            !/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/.test(
                client.clientId,
            ) ||
            client.clientId === "00000000-0000-0000-0000-000000000000" ||
            !Array.isArray(client.scopes) ||
            !client.scopes.includes("openid") ||
            client.scopes.some(
                (scope) =>
                    typeof scope !== "string" || !scope || /\s/.test(scope),
            ) ||
            new Set(client.scopes).size !== client.scopes.length
        )
            throw new SetupEvidenceError("Setup evidence is invalid");
        login = {
            client_id: client.clientId,
            scopes: client.scopes as string[],
        };
    } else if ("loginClient" in value && value.loginClient != null) {
        throw new SetupEvidenceError("Setup evidence is invalid");
    }
    return {
        stage: value.ready
            ? "ready"
            : value.state === "unavailable"
              ? "unavailable"
              : "installation",
        state: value.state as SetupStatus["state"],
        ready: value.ready,
        owners: owners.filter((owner) =>
            (value.owners as unknown[]).includes(owner),
        ),
        ...(login ? { login_client: login } : {}),
    };
}

export async function fetchSetupHosts(bases: SetupBases): Promise<SetupHost[]> {
    const base = setupBase(bases.idp);
    const status = await fetchSetupStatus(base);
    if (status.owners.length === 3) return [{ base, status }];
    if (status.owners[0] !== "idp")
        throw new Error("Primary setup scope must include IdP");
    if (!bases.device || !bases.storage)
        throw new SetupPrerequisiteError(
            "Independent setup requires configured Management and Storage HTTPS service bases",
        );
    const device = setupBase(bases.device);
    const storage = setupBase(bases.storage);
    if (new Set([base, device, storage]).size !== 3)
        throw new SetupPrerequisiteError(
            "Independent setup service bases must be distinct",
        );
    const hosts: SetupHost[] = [{ base, status }];
    for (const [owner, target] of [
        ["device", device],
        ["storage", storage],
    ] as const) {
        const targetStatus = await fetchSetupStatus(target);
        if (
            targetStatus.owners.length !== 1 ||
            targetStatus.owners[0] !== owner
        )
            throw new SetupEvidenceError(
                `Setup scope does not match the configured ${owner} service`,
            );
        hosts.push({ base: target, status: targetStatus });
    }
    if (hosts.every((host) => host.status.ready)) {
        const { verifyIndependentReadiness, independentLoginClient } =
            await import("./independentSetupApi.ts");
        const evidence = await verifyIndependentReadiness(hosts);
        hosts[0].status.login_client = independentLoginClient(evidence[0]);
    }
    return hosts;
}

function requireSetupCode(code: string): void {
    if (!/^[a-f0-9]{64}$/.test(code))
        throw new Error("Enter the exact host-generated setup code");
}

function requireHumanIntent(nonce: string, names: string[]): void {
    if (
        !/^[a-f0-9]{32}$/.test(nonce) ||
        !names.every(
            (value) =>
                typeof value === "string" &&
                value === value.trim() &&
                new TextEncoder().encode(value).byteLength <= 128 &&
                /^\P{Cc}+$/u.test(value),
        )
    )
        throw new Error("Invalid exact setup intent");
}

function exactCreateIntent(intent: SetupCreateIntent): SetupCreateIntent {
    requireHumanIntent(intent.operation_nonce, [
        intent.administrator,
        intent.node,
    ]);
    return {
        operation_nonce: intent.operation_nonce,
        administrator: intent.administrator,
        node: intent.node,
    };
}

export async function submitSetupCreate(
    base: string,
    code: string,
    request: SetupCreateRequest,
): Promise<SetupStatus> {
    const intent = exactCreateIntent(request);
    if (
        typeof request.password !== "string" ||
        new TextEncoder().encode(request.password).byteLength > 1024 ||
        !/^\P{Cc}+$/u.test(request.password)
    )
        throw new Error("Invalid protected administrator password");
    const response = await setupClient(base, code).setupCreateRaw(
        {
            createRequest: {
                operationNonce: intent.operation_nonce,
                administrator: intent.administrator,
                node: intent.node,
                password: request.password,
            },
        },
        setupInit,
    );
    if (response.raw.status !== 202 && response.raw.status !== 200)
        throw new Error(
            `Setup Create rejected (${response.raw.status}); observe status before an exact retry`,
        );
    const status = await readSetupStatus(
        await response.value(),
        response.raw.status,
    );
    if (
        status.owners.length !== 3 ||
        (response.raw.status === 202 &&
            (status.ready || status.state !== "creating")) ||
        (response.raw.status === 200 && !status.ready)
    )
        throw new SetupEvidenceError("Setup evidence is invalid");
    return status;
}

export async function cancelSetupCreate(
    base: string,
    code: string,
    intent: SetupCreateIntent,
): Promise<void> {
    const exact = exactCreateIntent(intent);
    const response = await setupClient(base, code).setupCancelRaw(
        {
            setupIntent: {
                operationNonce: exact.operation_nonce,
                administrator: exact.administrator,
                node: exact.node,
            },
        },
        setupInit,
    );
    if (response.raw.status !== 204)
        throw new Error(
            `Setup cancellation rejected (${response.raw.status}); observe status`,
        );
}

export async function fetchSetupIntent(
    base: string,
    code: string,
): Promise<SetupIntent | null> {
    const response = await setupClient(base, code).setupIntentRaw(setupInit);
    if (response.raw.status === 204) return null;
    if (response.raw.status !== 200)
        throw new Error(
            `Setup intent recovery rejected (${response.raw.status})`,
        );
    try {
        const value: unknown = SetupIntentToJSON(await response.value());
        if (!value || typeof value !== "object" || Array.isArray(value))
            throw new Error("Invalid retained setup intent");
        const keys = Object.keys(value).sort().join(",");
        if (keys === "administrator,node,operation_nonce")
            return exactCreateIntent(value as SetupCreateIntent);
        if (keys === "issuer,node,operation_nonce,peer")
            return exactJoinIntent(value as SetupJoinIntent);
        throw new Error("Invalid retained setup intent");
    } catch {
        throw new Error("Invalid retained setup intent");
    }
}

export function exactJoinRequest(request: SetupJoinRequest): SetupJoinRequest {
    requireHumanIntent(request.operation_nonce, [request.node]);
    if (!/^[a-f0-9]{64}$/.test(request.peer) || /^0+$/.test(request.peer))
        throw new Error("Invalid canonical pinned peer");
    return {
        operation_nonce: request.operation_nonce,
        node: request.node,
        peer: request.peer,
    };
}

function exactJoinIntent(intent: SetupJoinIntent): SetupJoinIntent {
    const request = exactJoinRequest(intent);
    if (
        typeof intent.issuer !== "string" ||
        intent.issuer !== intent.issuer.trim()
    )
        throw new Error("Invalid retained Join issuer");
    setupBase(intent.issuer);
    return { ...request, issuer: intent.issuer };
}

export async function submitSetupJoin(
    base: string,
    code: string,
    request: SetupJoinRequest,
): Promise<SetupStatus> {
    const intent = exactJoinRequest(request);
    const response = await setupClient(base, code).setupJoinRaw(
        {
            joinRequest: {
                operationNonce: intent.operation_nonce,
                node: intent.node,
                peer: intent.peer,
            },
        },
        setupInit,
    );
    if (response.raw.status !== 202)
        throw new Error(
            `Setup Join rejected (${response.raw.status}); observe status before an exact retry`,
        );
    const status = await readSetupStatus(
        await response.value(),
        response.raw.status,
    );
    if (
        status.owners.length !== 3 ||
        status.ready ||
        status.state !== "joining"
    )
        throw new SetupEvidenceError("Setup evidence is invalid");
    return status;
}

export async function cancelSetupJoin(
    base: string,
    code: string,
    intent: SetupJoinIntent,
): Promise<void> {
    const exact = exactJoinIntent(intent);
    const response = await setupClient(base, code).setupCancelRaw(
        {
            setupIntent: {
                operationNonce: exact.operation_nonce,
                node: exact.node,
                peer: exact.peer,
                issuer: exact.issuer,
            },
        },
        setupInit,
    );
    if (response.raw.status !== 204)
        throw new Error(
            `Setup cancellation rejected (${response.raw.status}); observe status`,
        );
}

export async function verifySetupCode(
    base: string,
    code: string,
    independent = false,
): Promise<void> {
    requireSetupCode(code);
    const api = new IndependentApi(
        new IndependentConfiguration({
            basePath: setupBase(base),
            fetchApi: fetch,
            headers: { Authorization: `Setup ${code}` },
        }),
    );
    const response = independent
        ? await api.independentSetupAdmissionRaw(setupInit)
        : await setupClient(base, code).setupAdmissionRaw(setupInit);
    if (response.raw.status !== 204)
        throw new Error(`Setup admission rejected (${response.raw.status})`);
}
