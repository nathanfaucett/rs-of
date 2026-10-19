import { error } from "@sveltejs/kit";
import { invoke, isTauri } from "@tauri-apps/api/core";
import {
  getSetupStage,
  getWebSetupHosts,
  usesRemoteApis,
} from "$lib/common/state/setupClient.svelte";

export async function load() {
  const bundled = isTauri() && !usesRemoteApis();
  if (bundled) {
    try {
      const nativeError = await invoke<string | null>("get_native_host_error");
      if (nativeError)
        return { stage: "unavailable", bundled: true, nativeError };
    } catch {
      return {
        stage: "unavailable",
        bundled: true,
        nativeError:
          "Native host observation unavailable; retry local prerequisites or explicit recovery.",
      };
    }
  }
    try {
      const hosts = await getWebSetupHosts();
    const stage = hosts.every((host) => host.status.ready)
      ? bundled
        ? await getSetupStage()
        : "ready"
          : hosts.some((host) => host.status.stage === "unavailable")
            ? "unavailable"
        : "installation";
    if (!["installation", "unavailable", "ready", "device"].includes(stage))
      throw new Error("Setup evidence is invalid; readiness not established");
    return bundled ? { stage, hosts, bundled: true } : { stage, hosts };
  } catch (cause) {
    if (bundled)
      return {
        stage: "unavailable",
        bundled: true,
        nativeError:
          "Bundled HTTPS setup unavailable; explicitly retry host prerequisites or recover/reset local references.",
      };
      throw error(
        503,
        cause instanceof Error ? cause.message : "Setup status unavailable",
      );
    }
  }
