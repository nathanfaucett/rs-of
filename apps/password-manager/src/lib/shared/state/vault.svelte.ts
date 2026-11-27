import { goto } from "$app/navigation";
import { listLegacyRecovery, unlockLegacyRecovery } from "$lib/vault/legacy-history";
import { createInactivityLock, type LockSettings } from "$lib/vault/lock";
import type { VaultHandleLike } from "$lib/vault/types";
import {
  createVault as createVaultHandle,
  listVaults,
  unlockVault,
  type VaultIdentity,
} from "$lib/vault/vault";
import { syncStateModule } from "./sync.svelte";

type VaultChoice = VaultIdentity & { recoveryResource?: string };

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
      recovery: boolean;
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
      recovery: vaultState.recovery,
    };
  }
  return null;
}

async function showRecoveryVaults(id: string) {
  const previous = vaultState;
  if (previous.status !== "select-vault") return;
  const source = await listLegacyRecovery(id);
  if (vaultState === previous) {
    vaultState = {
      ...previous,
      vaults: [
        ...previous.vaults.filter((entry) => !entry.recoveryResource),
        ...source.map((entry) => ({ ...entry, recoveryResource: id })),
      ],
    };
  }
  return source.length;
}

function activateVault(vault: VaultHandleLike, lockSettings: LockSettings, recovery: boolean) {
  if (vaultState.status === "unlocked") vaultState.inactivity.stop();
  const owner = ++session;
  const inactivity = createInactivityLock(() => {
    if (session === owner && vaultState.status === "unlocked") void lockVault();
  });
  vaultState = { status: "unlocked", vault, lockSettings, inactivity, recovery };
  inactivity.start(lockSettings);
}

async function createVault(password: string, name: string) {
  const request = session;
  const vault = await createVaultHandle(password, name.trim());
  const settings = await vault.loadLockSettings();
  if (request !== session) {
    await vault.close();
    return;
  }
  activateVault(vault, settings, false);
  await goto("/vault");
}

async function unlockExistingVault(password: string, { id, recoveryResource }: VaultChoice) {
  const request = session;
  const vault = recoveryResource
    ? await unlockLegacyRecovery(recoveryResource, id, password)
    : await unlockVault(password, id);
  let activated = false;
  try {
    if (request !== session) return;
    const settings = await vault.loadLockSettings();
    if (!recoveryResource) await syncStateModule.verifyUpgradedVault(vault);
    if (request !== session) return;
    activateVault(vault, settings, !!recoveryResource);
    activated = true;
    await goto("/vault");
  } finally {
    if (!activated) await vault.close();
  }
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
  showRecoveryVaults,
  init,
};
