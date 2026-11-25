import { redirect } from "@sveltejs/kit";
import { nodeSetupStatus } from "$lib/common/state/nodeSetup";
import { observeOnboarding } from "$lib/common/state/onboarding";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ fetch }) => {
  const node = await nodeSetupStatus(fetch);
  if (node?.route === "/setup") return { state: node.state };
  if (node) redirect(302, node.route);
  const observation = await observeOnboarding(node, fetch);
  throw redirect(302, observation.state === "ready" ? "/signin" : "/setup/installation");
};
