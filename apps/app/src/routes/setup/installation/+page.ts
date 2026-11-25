import { redirect } from "@sveltejs/kit";
import { isTauri } from "@tauri-apps/api/core";
import { getIdpApiUrl } from "$lib/common/state/idpClient.svelte";
import { nodeSetupStatus } from "$lib/common/state/nodeSetup";
import { observeOnboarding } from "$lib/common/state/onboarding";

export async function load() {
  if (!isTauri() && !getIdpApiUrl()) throw redirect(302, "/config");
  const node = await nodeSetupStatus();
  if (node?.route === "/setup/installation") return { unified: true } as const;
  if (node && node.route !== "/signin") redirect(302, node.route);
  const observation = await observeOnboarding(node);
  if (node && observation.state === "ready") redirect(302, "/signin");
  if (
    observation.target.kind === "remote" &&
    observation.state === "blocked" &&
    ["retry", "prerequisite"].includes(observation.action)
  )
    throw redirect(302, "/config");
  return { unified: false, observation } as const;
}
