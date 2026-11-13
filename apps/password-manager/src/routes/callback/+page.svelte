<script lang="ts">
import { onMount } from "svelte";
import { goto } from "$app/navigation";
import { Alert, Card, Page, PageHeader } from "$lib/ui";
import { authState } from "$lib/auth/auth.svelte";

let status = $state<"processing" | "success" | "error">("processing");
let message = $state("");

onMount(async () => {
  try {

    if (authState.isAuthenticated) {
      status = "success";
      message = "Sign in successful. Redirecting…";
      await goto("/");
    } else {
      status = "error";
      message = authState.error || "Authentication failed";
    }
  } catch (e) {
    status = "error";
    message = e instanceof Error ? e.message : "Authentication failed";
  }
});
</script>

<div class="flex min-h-screen items-center justify-center p-6 bg-background">
  <Page width="narrow">
    <PageHeader title="Password Manager" description="Completing sign in…" />
    <Card>
      <div class="flex flex-col gap-4 items-center text-center">
        {#if status === "processing"}
          <div class="animate-spin rounded-full h-8 w-8 border-2 border-primary border-t-transparent"></div>
          <p>Completing sign in…</p>
        {:else if status === "success"}
          <svg class="h-12 w-12 text-green-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
          </svg>
          <p>{message}</p>
        {:else}
          <svg class="h-12 w-12 text-red-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
          <Alert tone="danger">{message}</Alert>
          <a href="/login" class="btn btn-secondary btn-md">Try again</a>
        {/if}
      </div>
    </Card>
  </Page>
</div>
