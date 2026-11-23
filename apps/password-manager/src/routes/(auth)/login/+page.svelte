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
  } catch {
    // Error handled by authState.error
  } finally {
    busy = false;
  }
}

onMount(() => {
  if (authState.isAuthenticated) {
    void goto(resolve("/"));
  }
});</script>

<svelte:window onfocus={() => {}} />

<div class="flex min-h-screen items-center justify-center p-6 bg-background">
  <Page width="narrow">
    <PageHeader title="Password Manager" description="Sign in to access your vaults across devices." />
    <Card>
      <div class="flex flex-col gap-4">
        {#if authState.error}
          <Alert tone="danger">{authState.error}</Alert>
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
