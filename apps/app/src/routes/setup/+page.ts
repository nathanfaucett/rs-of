import { redirect } from "@sveltejs/kit";
import { observeOnboarding } from "$lib/common/state/onboarding";

export async function load() {
    const observation = await observeOnboarding();
    throw redirect(
        302,
        observation.state === "ready"
            ? "/signin"
            : "/setup/installation",
    );
}
