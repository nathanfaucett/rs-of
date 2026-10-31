<script lang="ts">
import "../app.css";

import { getTheme } from "@aicacia/svelte-headless";
import favicon from "$lib/assets/favicon.svg";
import Notifications from "$lib/common/components/Notifications.svelte";
import type { LayoutProps } from "./$types";

let { children }: LayoutProps = $props();

$effect(() => {
    if (getTheme() === "dark") {
        document.documentElement.classList.add("dark");
    } else {
        document.documentElement.classList.remove("dark");
    }
});
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

{@render children()}
<Notifications />
