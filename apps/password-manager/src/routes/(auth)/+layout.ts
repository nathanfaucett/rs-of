import { redirect } from "@sveltejs/kit";
import { resolve } from "$app/paths";
import { afterSigninRedirect } from "$lib/common/state/afterSigninRedirect.svelte";
import { getOidcClient } from "$lib/common/state/oidc.svelte";
import type { LayoutLoad } from "./$types";

export const load: LayoutLoad = async (event) => {
    await event.parent();

    try {
        const currentUserInfo = await getOidcClient().getUserInfo();

        if (currentUserInfo) {
            return { userInfo: currentUserInfo };
        }
    } catch {}

    afterSigninRedirect.setURL(event.url);
    redirect(302, resolve("/signin"));
};
