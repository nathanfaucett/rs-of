import { redirect } from "@sveltejs/kit";
import { observeOnboarding } from "$lib/common/state/onboarding";

export async function load() {
    const observation = await observeOnboarding();
    throw redirect(
        302,
        observation.state === "ready"
            ? observation.action === "device_setup"
                ? "/setup/device"
                : "/signin"
            : "/setup/installation",
    );
}
