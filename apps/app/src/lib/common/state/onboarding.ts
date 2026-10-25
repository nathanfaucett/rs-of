import { CreateResultFromJSON } from "@aicacia/idp-client";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { env } from "$env/dynamic/public";
import { getIdpApiUrl, usesRemoteApis } from "./idpClient.svelte";
import { getDeviceApiUrl } from "./deviceClient.svelte";
import {
    fetchSetupHosts,
    readSetupStatus,
    requireSetupUiOrigin,
    setupBase,
    SetupEvidenceError,
    SetupPrerequisiteError,
    type SetupHost,
} from "./setupApi";

export type OnboardingTarget = {
    kind: "bundled" | "remote";
    idp: string;
    device: string | null;
    storage: string | null;
};
export type OnboardingFailure = "retry" | "prerequisite" | "repair" | "trust_consent" | "trust_declined";
type Evidence = { target: OnboardingTarget; hosts: SetupHost[] };
export type OnboardingObservation = Evidence &
    (
        | { state: "starting"; action: "observe" }
        | { state: "needs_setup"; action: "choose" }
        | { state: "working"; action: "observe" | "approve" }
        | {
              state: "ready";
              action: "sign_in" | "device_setup";
              loginClient: { client_id: string; scopes: string[] };
          }
        | { state: "blocked"; action: OnboardingFailure; message: string }
    );

type NativeHostObservation = {
    base?: string;
    status?: unknown;
    device_setup_required?: boolean;
    failure?: OnboardingFailure;
};

function selection(): OnboardingTarget {
    return {
        kind: isTauri() && !usesRemoteApis() ? "bundled" : "remote",
        idp: getIdpApiUrl()?.replace(/\/$/, "") ?? "",
        device: getDeviceApiUrl() ?? null,
        storage: env.PUBLIC_STORAGE_BASE_URL ?? null,
    };
}
function sameSelection(target: OnboardingTarget): boolean {
    const current = selection();
    return (
        target.kind === current.kind &&
        target.idp === current.idp &&
        target.device === current.device &&
        target.storage === current.storage
    );
}
function publicTarget(target: OnboardingTarget): OnboardingTarget {
    function base(value: string | null): string | null {
        try {
            return value ? setupBase(value) : null;
        } catch {
            return null;
        }
    }
    return {
        kind: target.kind,
        idp: base(target.idp) ?? "",
        device: base(target.device),
        storage: base(target.storage),
    };
}

function blocked(
    target: OnboardingTarget,
    action: OnboardingFailure,
    hosts: SetupHost[] = [],
): OnboardingObservation {
    return {
        target: publicTarget(target),
        hosts,
        state: "blocked",
        action,
        message:
            action === "trust_consent"
                ? "The retained localhost certificate is not currently trusted by the operating system. Trust consent is required before local HTTPS can start."
                : action === "trust_declined"
                  ? "Certificate trust was declined. Retained identity is unchanged. Retry local services to reconsider, or select remote HTTPS services in Connection settings."
                : action === "repair"
                ? "Retained setup evidence needs repair. Preserve the existing identity; use exact recovery, not replacement or reset."
                : action === "prerequisite"
                  ? "Check protected-provider access, HTTPS trust and selected service configuration, then observe again."
                  : "Unable to verify current readiness. Check the connection and retry observation; retained work is unchanged.",
    };
}

export async function observeOnboarding(): Promise<OnboardingObservation> {
    const selected = selection();
    let target = publicTarget(selected);
    let hosts: SetupHost[];
    let deviceSetup = false;
    try {
        if (selected.kind === "bundled") {
            const native = await invoke<NativeHostObservation>(
                "get_onboarding_host",
            );
            if (!sameSelection(selected)) return blocked(selection(), "retry");
            if (native.failure)
                return blocked(
                    target,
                    ["retry", "prerequisite", "repair", "trust_consent", "trust_declined"].includes(native.failure)
                        ? native.failure
                        : "repair",
                );
            if (!native.base)
                return {
                    target,
                    hosts: [],
                    state: "starting",
                    action: "observe",
                };
            const base = setupBase(native.base);
            target = {
                kind: "bundled",
                idp: `${base}/idp`,
                device: `${base}/device`,
                storage: `${base}/storage`,
            };
            const status = readSetupStatus(
                CreateResultFromJSON(native.status),
                200,
            );
            if (
                status.ready &&
                typeof native.device_setup_required !== "boolean"
            )
                return blocked(target, "repair");
            if (status.owners.length !== 3) return blocked(target, "repair");
            hosts = [{ base: target.idp, status }];
            deviceSetup = native.device_setup_required === true;
        } else {
            requireSetupUiOrigin(
                typeof window === "undefined" ? "" : window.location.origin,
            );
            if (!selected.idp) return blocked(target, "prerequisite");
            hosts = await fetchSetupHosts(selected);
            if (!sameSelection(selected)) return blocked(selection(), "retry");
        }
    } catch (cause) {
        if (!sameSelection(selected)) return blocked(selection(), "retry");
        return blocked(
            target,
            cause instanceof SetupPrerequisiteError
                ? "prerequisite"
                : cause instanceof SetupEvidenceError
                  ? "repair"
                  : "retry",
        );
    }
    if (hosts.some((host) => host.status.state === "unavailable"))
        return blocked(target, "prerequisite", hosts);
    if (hosts.some((host) => host.status.state === "unfinished"))
        return blocked(target, "repair", hosts);
    if (hosts.every((host) => host.status.ready)) {
        const loginClient = hosts[0]?.status.login_client;
        if (!loginClient) return blocked(target, "repair", hosts);
        return {
            target,
            hosts,
            state: "ready",
            action: deviceSetup ? "device_setup" : "sign_in",
            loginClient,
        };
    }
    if (
        hosts.some((host) =>
            ["creating", "joining"].includes(host.status.state),
        )
    )
        return {
            target,
            hosts,
            state: "working",
            action: hosts.some((host) => host.status.state === "joining")
                ? "approve"
                : "observe",
        };
    return { target, hosts, state: "needs_setup", action: "choose" };
}
