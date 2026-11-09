export const prerender = false;
export const ssr = false;

import { authState } from "$lib/auth/auth.svelte";

// Initialize auth on app load
authState.initialize();
