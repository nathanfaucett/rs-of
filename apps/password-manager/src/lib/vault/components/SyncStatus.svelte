<script lang="ts">
import { authState } from "$lib/auth/auth.svelte";
import { vaultStateModule } from "$lib/shared/state/vault.svelte";
import { Card } from "$lib/ui";
</script>

<Card class="w-full">
  <h3 class="text-lg font-medium mb-4">Sync Status</h3>
  <p class="text-sm text-muted-foreground">
    {#if vaultStateModule.vaultData?.recovery}
      This recovery copy is read-only. Export an encrypted backup to keep working.
    {:else if !authState.isAuthenticated}
      Your vault is stored on this device. Sign in to your Sync Server to manage sync.
    {:else}
      Lock the vault to view or change sync in the vault selector.
    {/if}
  </p>
</Card>
