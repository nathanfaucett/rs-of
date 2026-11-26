<script lang="ts">
import { onMount } from "svelte";
import { goto } from "$app/navigation";
import { resolve } from "$app/paths";
import { authState } from "$lib/auth/auth.svelte";
import { Alert, Card, Field, Page, PageHeader } from "$lib/ui";

let busy = $state(false);

async function handleLogin() {
  if (busy) return;
  busy = true;
  try {
    await authState.login();
  } finally {
    busy = false;
  }
}

onMount(() => {
  void authState.initialize().then(() => {
    if (authState.isAuthenticated) void goto(resolve("/"));
  });
});
</script>

<svelte:head><title>Sign in | Password Manager</title></svelte:head>

<div class="flex min-h-screen items-center justify-center p-6 bg-background">
  <Page width="narrow">
    <PageHeader title="Password Manager" description="Sign in to access your vaults across devices." />
    <Card>
      <div class="flex flex-col gap-4">
        {#if authState.error}
          <Alert tone="danger">{authState.error}</Alert>
        {/if}
        {#if authState.setupUrl}
          <a class="btn btn-secondary btn-md" href={authState.setupUrl}>Open Node setup</a>
        {/if}
        <Field id="login" label="Sign in with your identity provider">
          <div class="flex flex-col gap-3">
            <button class="btn btn-primary btn-md w-full" type="button" disabled={busy} onclick={handleLogin}>
              {busy ? "Redirecting…" : "Sign in with IdP"}
            </button>
            <p class="text-sm text-muted-foreground text-center">
              You will be redirected to your identity provider to authenticate.
            </p>
          </div>
        </Field>
      </div>
    </Card>
  </Page>
</div>
