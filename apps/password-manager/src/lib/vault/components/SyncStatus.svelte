<script lang="ts">
import { authState } from "$lib/auth/auth.svelte";
import { syncStateModule } from "$lib/shared/state/sync.svelte";
import { vaultStateModule } from "$lib/shared/state/vault.svelte";
import { Alert, Card } from "$lib/ui";
</script>

<Card class="w-full">
  <h3 class="text-lg font-medium mb-4">Sync Status</h3>
  <div class="flex flex-col gap-4">
    <div class="flex items-center gap-2">
      <span class={[
        "inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium",
        {
          "tone-success": vaultStateModule.vaultData?.isSynced,
          "tone-warning": !vaultStateModule.vaultData?.isSynced,
        },
      ]}>
        {#if vaultStateModule.vaultData?.isSynced}
          <span class="flex items-center gap-1">
            <span class="w-2 h-2 rounded-full bg-current animate-pulse"></span>
            Synced
          </span>
        {:else}
          Local only
        {/if}
      </span>
    </div>

    {#if vaultStateModule.vaultData?.isSynced}
      <p class="text-sm text-muted-foreground">
        Select this vault through the Device service to replicate it to an approved Device.
      </p>
    {:else if !authState.isAuthenticated}
      <p class="text-sm text-muted-foreground">
        Your vault is stored on this device. Sign in to your Sync Server to enable synchronization.
      </p>
    {:else if syncStateModule.isInitializing}
      <p class="text-sm text-muted-foreground">Connecting to the Sync Server…</p>
    {:else if syncStateModule.initError}
      <Alert tone="danger">{syncStateModule.initError}</Alert>
    {:else}
      <p class="text-sm text-muted-foreground">Signed in. Sync is not enabled for this vault.</p>
    {/if}
  </div>
</Card>
