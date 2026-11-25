import { goto } from "$app/navigation";
import { createInactivityLock, type LockSettings } from "$lib/vault/lock";
import { createSyncVault, listSyncVaults, unlockSyncVault } from "$lib/vault/sync-vault";
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
  | { status: "create-vault"; synced?: boolean }
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
  const synced = handle ? await listSyncVaults(handle) : [];
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

async function createVault(password: string, name: string, synced = false) {
  if (synced) {
    const handle =
      (await openMainDatabase()) ?? (await syncStateModule.createDatabase("password-manager-main"));
    const vault = await createSyncVault(password, name.trim(), handle);
    const request = session;
    const settings = await vault.loadLockSettings();
    const inactivity = createInactivityLock(() => {
      void lockVault();
    });
    inactivity.start(settings);
    if (request !== session) {
      await vault.close();
      return;
    }
    await handle.sync({ direction: "bidirectional" });
    vaultState = { status: "unlocked", vault, lockSettings: settings, inactivity, isSynced: true };
    await goto("/vault");
    return;
  }
  const vault = await createVaultHandle(password, name.trim());
  const request = session;
  const settings = await vault.loadLockSettings();
  const inactivity = createInactivityLock(() => {
    void lockVault();
  });
  inactivity.start(settings);
  if (request !== session) {
    await vault.close();
    return;
  }
  vaultState = { status: "unlocked", vault, lockSettings: settings, inactivity, isSynced: false };
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
    const inactivity = createInactivityLock(() => {
      void lockVault();
    });
    inactivity.start(settings);
    if (request !== session) {
      await vault.close();
      return;
    }
    await handle.sync({ direction: "bidirectional" });
    vaultState = { status: "unlocked", vault, lockSettings: settings, inactivity, isSynced: true };
    await goto("/vault");
    return;
  }
  const vault = await unlockVault(password, id);
  const request = session;
  const settings = await vault.loadLockSettings();
  const inactivity = createInactivityLock(() => {
    void lockVault();
  });
  inactivity.start(settings);
  if (request !== session) {
    await vault.close();
    return;
  }
  vaultState = { status: "unlocked", vault, lockSettings: settings, inactivity, isSynced: false };
  await goto("/vault");
}

async function lockVault() {
  const previous = vaultState;
  const request = ++session;
  vaultState = { status: "locked" };
  if (previous.status === "unlocked") {
    previous.inactivity.stop();
    await previous.vault.close();
  }
  if (request === session && vaultState.status === "locked") await goto("/");
}

async function showVaultSelector() {
  if (selector !== null) return selector;
  selector = (async () => {
    vaultState = { status: "loading" };
    try {
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
  lockVault,
  showVaultSelector,
  showSyncedVaults,
  init,
};
