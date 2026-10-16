import { error } from "@sveltejs/kit";

import { isTauri } from "@tauri-apps/api/core";

import {
  getSetupStage,
  getWebSetupHosts,
} from "$lib/common/state/setupClient.svelte";

export async function load() {
  if (!isTauri()) {
    try {
      const hosts = await getWebSetupHosts();
      return {
        stage: hosts.every((host) => host.status.ready)
          ? "ready"
          : hosts.some((host) => host.status.stage === "unavailable")
            ? "unavailable"
            : "installation",
        hosts,
      };
    } catch (cause) {
      throw error(
        503,
        cause instanceof Error ? cause.message : "Setup status unavailable",
      );
    }
  }
  const stage = await getSetupStage();
  if (
    stage !== "installation" &&
    stage !== "unavailable" &&
    stage !== "ready" &&
    stage !== "device"
  ) {
    throw error(
      503,
      "Local setup evidence is invalid. Readiness has not been established.",
    );
  }
  return { stage };
}
