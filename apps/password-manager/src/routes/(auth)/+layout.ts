import { redirect } from "@sveltejs/kit";
import { resolve } from "$app/paths";
import { afterSigninRedirect } from "$lib/common/state/afterSigninRedirect.svelte";
import { getSdk } from "$lib/common/state/sdk";
import type { LayoutLoad } from "./$types";

export const load: LayoutLoad = async (event) => {
    await event.parent();

    try {
        const sdk = getSdk();
        await sdk.restore();
        if (sdk.session.status === "authenticated" && sdk.session.identity) {
            return { userInfo: { sub: sdk.session.identity.subject } };
        }
    } catch {}

    afterSigninRedirect.setURL(event.url);
    redirect(302, resolve("/signin"));
};
