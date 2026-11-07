import { unlockVault, type VaultHandle, listVaults, createVault as createVaultHandle, type VaultIdentity } from "$lib/vault/vault";
import { createInactivityLock, DEFAULT_LOCK_SETTINGS, type LockSettings } from "$lib/vault/lock";
import { notifications } from "./notifications.svelte";
import { goto } from "$app/navigation";

type VaultState =
    | { status: "loading" }
    | { status: "locked" }
    | { status: "select-vault"; vaults: VaultIdentity[] }
    | { status: "create-vault" }
    | { status: "restore-vault" }
    | { status: "unlock-vault"; identity: VaultIdentity }
    | { status: "unlocked"; vault: VaultHandle; lockSettings: LockSettings; inactivity: ReturnType<typeof createInactivityLock> };

let vaultState = $state<VaultState>({ status: "loading" });
let session = 0;

function getVaultData() {
    if (vaultState.status === "unlocked") {
        return {
            vault: vaultState.vault,
            lockSettings: vaultState.lockSettings,
            inactivity: vaultState.inactivity,
        };
    }
    return null;
}

async function loadVaults() {
    const vaults = await listVaults();
    return vaults;
}

async function enterVault(vault: VaultHandle, request: number, password: string) {
    const settings = await vault.loadLockSettings();
    if (request !== session) { vault.close(); return; }
    const inactivity = createInactivityLock(() => { void lockVault(); });
    inactivity.start(settings);
    vaultState = { status: "unlocked", vault, lockSettings: settings, inactivity };
}

async function createVault(password: string, name: string) {
    const vault = await createVaultHandle(password, name.trim());
    const request = session;
    const settings = await vault.loadLockSettings();
    const inactivity = createInactivityLock(() => { void lockVault(); });
    inactivity.start(settings);
    if (request !== session) { vault.close(); return; }
    vaultState = { status: "unlocked", vault, lockSettings: settings, inactivity };
    await goto("/vault");
}

async function unlockExistingVault(password: string, id: string) {
    const vault = await unlockVault(password, id);
    const request = session;
    const settings = await vault.loadLockSettings();
    const inactivity = createInactivityLock(() => { void lockVault(); });
    inactivity.start(settings);
    if (request !== session) { vault.close(); return; }
    vaultState = { status: "unlocked", vault, lockSettings: settings, inactivity };
    await goto("/vault");
}

async function lockVault() {
    if (vaultState.status === "unlocked") {
        vaultState.inactivity.stop();
        vaultState.vault.close();
    }
    vaultState = { status: "locked" };
    session++;
    await goto("/");
}

async function showVaultSelector() {
    vaultState = { status: "loading" };
    const vaults = await loadVaults();
    vaultState = { status: "select-vault", vaults };
}

async function init() {
    await showVaultSelector();
}

export const vaultStateModule = {
    get state() { return vaultState; },
    set state(value: VaultState) { vaultState = value; },
    get vaultData() { return getVaultData(); },
    createVault,
    unlockExistingVault,
    lockVault,
    showVaultSelector,
    init,
};
