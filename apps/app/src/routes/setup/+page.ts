import { redirect } from "@sveltejs/kit";
import { nodeSetupStatus } from "$lib/common/state/nodeSetup";
import { observeOnboarding } from "$lib/common/state/onboarding";

export async function load() {
  const node = await nodeSetupStatus();
  if (node?.route === "/setup") return { state: node.state };
  if (node) redirect(302, node.route);
  const observation = await observeOnboarding();
  throw redirect(302, observation.state === "ready" ? "/signin" : "/setup/installation");
}
