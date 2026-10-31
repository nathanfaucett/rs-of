import { isTauri } from "@tauri-apps/api/core";
import {
    Configuration,
    OwnerSetupApi,
    type OwnerSetupStatus,
    OwnerSetupStatusFromJSON,
    JSONApiResponse,
    ResponseError,
    FetchError,
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
    const location =
        typeof window === "undefined" ? undefined : window.location;
    const origin =
        location?.origin === "null" &&
        location.protocol === "tauri:" &&
        location.host === "localhost"
            ? "tauri://localhost"
            : (location?.origin ?? "");
    if (
        isTauri() &&
        ["tauri://localhost", "http://tauri.localhost"].includes(origin)
    )
        return;
    requireSetupUiOrigin(origin);
}

const setupInit: RequestInit = {
    credentials: "omit",
    redirect: "error",
    cache: "no-store",
};

export async function fetchSetupStatus(base: string): Promise<SetupStatus> {
    const api = new OwnerSetupApi(
        new Configuration({ basePath: setupBase(base), fetchApi: fetch }),
    );
    try {
        const response = await api.ownerSetupStatusRaw(setupInit);
        if (response.raw.status !== 200)
            throw new SetupEvidenceError("Invalid owner status");
        return readSetupStatus(await response.value(), 200);
    } catch (error) {
        if (error instanceof ResponseError) {
            if (error.response.status === 503) {
                try {
                    return readSetupStatus(
                        await new JSONApiResponse(
                            error.response,
                            OwnerSetupStatusFromJSON,
                        ).value(),
                        503,
                    );
                } catch {
                    throw new SetupPrerequisiteError(
                        "Owner admission or readiness unavailable",
                    );
                }
            }
            if (error.response.status >= 500)
                throw new Error("Owner status unavailable; retry observation");
            throw new SetupPrerequisiteError(
                "Check selected owner configuration and status access",
            );
        }
        if (error instanceof FetchError)
            throw new Error(
                "Owner status request failed; verify HTTPS trust and UI origin",
                { cause: error },
            );
        throw new SetupEvidenceError("Owner setup evidence is invalid", {
            cause: error,
        });
    }
}
export function readSetupStatus(
    value: OwnerSetupStatus,
    responseStatus: number,
): SetupStatus {
    if (
        !value ||
        value.stage !== "installation" ||
        typeof value.ready !== "boolean" ||
        ![
            "awaiting_choice",
            "creating",
            "joining",
            "unfinished",
            "unavailable",
            "ready",
        ].includes(value.state) ||
        value.ready !== (value.state === "ready") ||
        (responseStatus === 503 && value.state !== "unavailable") ||
        !Array.isArray(value.owners) ||
        value.owners.length !== 1 ||
        !owners.includes(value.owners[0] as SetupOwner)
    )
        throw new SetupEvidenceError("Owner setup evidence is invalid");
    return {
        stage: value.ready
            ? "ready"
            : value.state === "unavailable"
              ? "unavailable"
              : "installation",
        state: value.state as SetupStatus["state"],
        ready: value.ready,
        owners: value.owners as SetupOwner[],
    };
}
export async function fetchSetupHosts(bases: SetupBases): Promise<SetupHost[]> {
    const base = setupBase(bases.idp);
    const status = await fetchSetupStatus(base);
    if (status.owners[0] !== "idp")
        throw new Error("Primary setup scope must include IdP");
    if (!bases.device || !bases.storage)
        throw new SetupPrerequisiteError(
            "Independent setup requires configured Device and Storage HTTPS service bases",
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

export async function verifySetupCode(
    base: string,
    code: string,
): Promise<void> {
    if (!/^[a-f0-9]{64}$/.test(code))
        throw new Error("Enter the exact owner setup code");
    const api = new OwnerSetupApi(
        new Configuration({
            basePath: setupBase(base),
            fetchApi: fetch,
            headers: { Authorization: `Setup ${code}` },
        }),
    );
    const response = await api.ownerSetupAdmissionRaw(setupInit);
    if (response.raw.status !== 204)
        throw new Error("Owner admission rejected");
}
