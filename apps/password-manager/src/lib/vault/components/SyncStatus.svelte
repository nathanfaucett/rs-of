<script lang="ts">
import { Alert, Card, Field } from "$lib/ui";
import { vaultStateModule } from "$lib/shared/state/vault.svelte";
import { syncStateModule } from "$lib/shared/state/sync.svelte";
import type { SyncVaultHandle } from "$lib/vault/sync-vault";

let targetDeviceId = $state("");
let isSyncing = $state(false);
let syncError = $state("");
let syncSuccess = $state("");

async function handleSyncToDevice() {
  if (!targetDeviceId.trim() || isSyncing) return;
  const vaultData = vaultStateModule.vaultData;
  if (!vaultData || !vaultData.isSynced) return;

  const vault = vaultData.vault as SyncVaultHandle;

  isSyncing = true;
  syncError = "";
  syncSuccess = "";

  try {
    await vault.syncToDevice(targetDeviceId.trim());
    syncSuccess = `Sync initiated to device ${targetDeviceId.trim()}`;
  } catch (e) {
    syncError = e instanceof Error ? e.message : "Sync failed";
  } finally {
    isSyncing = false;
  }
}

function getSyncStatusTone(status: string): "success" | "warning" | "danger" | "info" {
  switch (status) {
    case "synced": return "success";
    case "connecting": return "info";
    case "disconnected": return "warning";
    case "error": return "danger";
    default: return "info";
  }
}
</script>

<Card class="w-full">
  <h3 class="text-lg font-medium mb-4">Sync Status</h3>
  <div class="flex flex-col gap-4">
    <div class="flex items-center gap-2">
      <span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium"
        class:tone-success={vaultStateModule.vaultData?.isSynced}
        class:tone-warning={!vaultStateModule.vaultData?.isSynced}
      >
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
      <div class="border-t pt-4">
        <h4 class="font-medium mb-2">Sync to another device</h4>
        <p class="text-sm text-muted-foreground mb-3">
          Enter the target device ID to sync this vault to another approved device.
        </p>
        <Field id="target-device" label="Target Device ID">
          {#snippet children(control)}
            <div class="flex gap-2">
              <input
                {...control}
                bind:value={targetDeviceId}
                type="text"
                placeholder="device-uuid"
                class="flex-1"
                disabled={isSyncing}
              />
              <button class="btn btn-primary" type="button" disabled={isSyncing || !targetDeviceId.trim()} onclick={handleSyncToDevice}>
                {isSyncing ? "Syncing…" : "Sync to Device"}
              </button>
            </div>
          {/snippet}
        </Field>

        {#if syncError}
          <Alert tone="danger">{syncError}</Alert>
        {/if}
        {#if syncSuccess}
          <Alert tone="success">{syncSuccess}</Alert>
        {/if}
      </div>
    {:else}
      <p class="text-sm text-muted-foreground">
        Sign in to enable multi-device sync.
      </p>
    {/if}
  </div>
</Card>
