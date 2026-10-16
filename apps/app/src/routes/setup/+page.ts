import { error, redirect } from "@sveltejs/kit";
import { isTauri } from "@tauri-apps/api/core";

import { getSetupStage } from "$lib/common/state/setupClient.svelte";

export async function load() {
  const stage = await getSetupStage().catch((cause) => {
    if (isTauri()) throw cause;
    throw error(
      503,
      cause instanceof Error ? cause.message : "Setup status unavailable",
    );
  });
  throw redirect(
    302,
    stage === "ready"
      ? "/signin"
      : stage === "device"
        ? "/setup/device"
        : "/setup/installation",
  );
}
