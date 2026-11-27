import { goto } from "$app/navigation";
import { createInactivityLock, type LockSettings } from "$lib/vault/lock";
import { listSyncVaults, unlockSyncVault } from "$lib/vault/sync-vault";
import type { VaultHandleLike } from "$lib/vault/types";
import {
  createVault as createVaultHandle,
  listVaults,
  unlockVault,
  type VaultIdentity,
} from "$lib/vault/vault";
import { syncStateModule } from "./sync.svelte";

type VaultChoice = VaultIdentity & { synced?: boolean };

type VaultState =
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "locked" }
  | { status: "select-vault"; vaults: VaultChoice[] }
  | { status: "create-vault" }
  | { status: "restore-vault" }
  | { status: "unlock-vault"; identity: VaultChoice }
  | {
      status: "unlocked";
      vault: VaultHandleLike;
      lockSettings: LockSettings;
      inactivity: ReturnType<typeof createInactivityLock>;
      isSynced: boolean;
    };

let vaultState = $state<VaultState>({ status: "loading" });
let session = 0;
let selector: Promise<void> | null = null;

function getVaultData() {
  if (vaultState.status === "unlocked") {
    return {
      vault: vaultState.vault,
      lockSettings: vaultState.lockSettings,
      inactivity: vaultState.inactivity,
      isSynced: vaultState.isSynced,
    };
  }
  return null;
}

async function openMainDatabase() {
  const databases = await syncStateModule.listDatabases();
  const matching = databases.filter((entry) => entry.name === "password-manager-main");
  if (matching.length > 1)
    throw new Error("More than one Password Manager Resource exists. No vault data was changed.");
  return matching[0] ? syncStateModule.openDatabase(matching[0].id) : null;
}

async function showSyncedVaults() {
  const previous = vaultState;
  if (previous.status !== "select-vault") return;
  const handle = await openMainDatabase();
  const synced = handle ? await listSyncVaults(handle).finally(() => handle.close()) : [];
  if (vaultState === previous) {
    vaultState = {
      ...previous,
      vaults: [
        ...previous.vaults.filter((entry) => !entry.synced),
        ...synced.map((entry) => ({ ...entry, synced: true })),
      ],
    };
  }
  return synced.length;
}

function activateVault(vault: VaultHandleLike, lockSettings: LockSettings, isSynced: boolean) {
  if (vaultState.status === "unlocked") vaultState.inactivity.stop();
  const owner = ++session;
  const inactivity = createInactivityLock(() => {
    if (session === owner && vaultState.status === "unlocked") void lockVault();
  });
  vaultState = { status: "unlocked", vault, lockSettings, inactivity, isSynced };
  inactivity.start(lockSettings);
}

async function createVault(password: string, name: string) {
  const vault = await createVaultHandle(password, name.trim());
  const request = session;
  const settings = await vault.loadLockSettings();
  if (request !== session) {
    await vault.close();
    return;
  }
  activateVault(vault, settings, false);
  await goto("/vault");
}

async function unlockExistingVault(password: string, { id, synced }: VaultChoice) {
  if (synced) {
    const handle = await openMainDatabase();
    if (!handle) throw new Error("Failed to open sync database");
    const vault = await unlockSyncVault(password, id, handle);
    if (!vault) throw new Error("Wrong password or synced vault unavailable.");
    const request = session;
    const settings = await vault.loadLockSettings();
    if (request !== session) {
      await vault.close();
      return;
    }
    await handle.sync({ direction: "bidirectional" });
    activateVault(vault, settings, true);
    await goto("/vault");
    return;
  }
  const vault = await unlockVault(password, id);
  const request = session;
  const settings = await vault.loadLockSettings();
  if (request !== session) {
    await vault.close();
    return;
  }
  activateVault(vault, settings, false);
  await goto("/vault");
}

async function saveLockSettings(settings: LockSettings): Promise<boolean> {
  const current = vaultState;
  if (current.status !== "unlocked" || current.inactivity.check()) return false;

  const confirmed = await current.vault.saveLockSettings(settings);
  if (vaultState !== current || current.inactivity.check()) return false;

  current.inactivity.start(confirmed);
  current.lockSettings = confirmed;
  return true;
}

async function retireVault(status: "locked" | "loading") {
  const previous = vaultState;
  vaultState = { status };
  if (previous.status === "unlocked") {
    previous.inactivity.stop();
    await previous.vault.close();
  }
}

async function lockVault() {
  const request = ++session;
  await retireVault("locked");
  if (request === session && vaultState.status === "locked") await goto("/");
}

async function showVaultSelector() {
  if (selector !== null) return selector;
  ++session;
  selector = (async () => {
    try {
      await retireVault("loading");
      const vaults = await listVaults();
      vaultState = { status: "select-vault", vaults };
    } catch (cause) {
      vaultState = {
        status: "error",
        message: cause instanceof Error ? cause.message : "Failed to load vaults.",
      };
    } finally {
      selector = null;
    }
  })();
  return selector;
}

async function init() {
  await showVaultSelector();
}

export const vaultStateModule = {
  get state() {
    return vaultState;
  },
  set state(value: VaultState) {
    vaultState = value;
  },
  get vaultData() {
    return getVaultData();
  },
  createVault,
  unlockExistingVault,
  saveLockSettings,
  lockVault,
  showVaultSelector,
  showSyncedVaults,
  init,
};
