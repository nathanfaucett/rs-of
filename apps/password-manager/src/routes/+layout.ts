export const prerender = false;
export const ssr = false;

import { authState } from "$lib/auth/auth.svelte";

void authState.initialize();
