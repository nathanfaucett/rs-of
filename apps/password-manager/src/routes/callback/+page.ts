import { redirect } from "@sveltejs/kit";
import { resolve } from "$app/paths";
import { afterSigninRedirect } from "$lib/common/state/afterSigninRedirect.svelte";
import { notifications } from "$lib/common/state/notifications.svelte";
import { getSdk } from "$lib/common/state/sdk";
import type { PageLoad } from "./$types";

export const load: PageLoad = async (event) => {
    await event.parent();

    try {
        const oidcClient = getSdk();
        await oidcClient.handleSigninCallback(event.url);
    } catch (e) {
        if (e instanceof Error) {
            notifications.add(
                "Sign-in could not be completed. Please try again.",
            );
        }
        redirect(302, resolve("/signin"));
    }
    await afterSigninRedirect.onReturn();
};
