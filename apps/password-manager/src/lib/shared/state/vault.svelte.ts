import { unlockVault, unlockAndMigrateVault, type VaultHandle, listVaults, createVault as createVaultHandle, type VaultIdentity } from "$lib/vault/vault";
import { createSyncVault, unlockSyncVault, listSyncVaults, type SyncVaultHandle } from "$lib/vault/sync-vault";
import { createInactivityLock, DEFAULT_LOCK_SETTINGS, type LockSettings } from "$lib/vault/lock";
import { notifications } from "./notifications.svelte";
import { goto } from "$app/navigation";
import { syncStateModule } from "./sync.svelte";
import { authState } from "$lib/auth/auth.svelte";
import type { VaultHandleLike } from "$lib/vault/types";

type VaultState =
    | { status: "loading" }
    | { status: "locked" }
    | { status: "select-vault"; vaults: VaultIdentity[] }
    | { status: "create-vault" }
    | { status: "restore-vault" }
    | { status: "unlock-vault"; identity: VaultIdentity }
    | { status: "unlocked"; vault: VaultHandleLike; lockSettings: LockSettings; inactivity: ReturnType<typeof createInactivityLock>; isSynced: boolean };

let vaultState = $state<VaultState>({ status: "loading" });
let session = 0;

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

function isSyncEnabled(): boolean {
    return authState.isAuthenticated && !!syncStateModule.sdk;
}

async function loadVaults() {
    if (isSyncEnabled()) {
        const handle = await syncStateModule.openDatabase("password-manager-main");
        if (handle) {
            const vaults = await listSyncVaults(handle);
            handle.close();
            return vaults;
        }
    }
    const vaults = await listVaults();
    return vaults;
}

async function enterVault(vault: VaultHandleLike, request: number, password: string, isSynced: boolean) {
    const settings = await vault.loadLockSettings();
    if (request !== session) { vault.close(); return; }
    const inactivity = createInactivityLock(() => { void lockVault(); });
    inactivity.start(settings);
    vaultState = { status: "unlocked", vault, lockSettings: settings, inactivity, isSynced };
}

async function createVault(password: string, name: string) {
    if (isSyncEnabled()) {
        const handle = await syncStateModule.createDatabase("password-manager-main");
        if (!handle) throw new Error("Failed to create sync database");
        const syncSDK = syncStateModule.getSyncSDK();
        const vault = await createSyncVault(password, name.trim(), handle, syncSDK ? { syncToDevice: syncSDK.syncToDevice.bind(syncSDK) } : undefined);
        const request = session;
        const settings = await vault.loadLockSettings();
        const inactivity = createInactivityLock(() => { void lockVault(); });
        inactivity.start(settings);
        if (request !== session) { vault.close(); return; }
        await handle.sync({ direction: "bidirectional" });
        vaultState = { status: "unlocked", vault, lockSettings: settings, inactivity, isSynced: true };
        await goto("/vault");
        return;
    }
    const vault = await createVaultHandle(password, name.trim());
    const request = session;
    const settings = await vault.loadLockSettings();
    const inactivity = createInactivityLock(() => { void lockVault(); });
    inactivity.start(settings);
    if (request !== session) { vault.close(); return; }
    vaultState = { status: "unlocked", vault, lockSettings: settings, inactivity, isSynced: false };
    await goto("/vault");
}

async function unlockExistingVault(password: string, id: string) {
    if (isSyncEnabled()) {
        const handle = await syncStateModule.openDatabase("password-manager-main");
        if (!handle) throw new Error("Failed to open sync database");
        const syncSDK = syncStateModule.getSyncSDK();

        // Try to unlock as sync vault first
        let vault = await unlockSyncVault(password, id, handle, syncSDK ? { syncToDevice: syncSDK.syncToDevice.bind(syncSDK) } : undefined);
        let isSynced = true;

        // If sync vault doesn't exist, try to migrate local vault
        if (!vault) {
            const syncClient = {
                createDatabase: (name: string) => syncStateModule.createDatabase(name),
                openDatabase: (id: string) => syncStateModule.openDatabase(id),
            };
            const result = await unlockAndMigrateVault(password, id, syncClient);
            vault = result.vault;
            isSynced = result.isSynced;
        }

        const request = session;
        const settings = await vault.loadLockSettings();
        const inactivity = createInactivityLock(() => { void lockVault(); });
        inactivity.start(settings);
        if (request !== session) { vault.close(); return; }
        await handle.sync({ direction: "bidirectional" });
        vaultState = { status: "unlocked", vault, lockSettings: settings, inactivity, isSynced };
        await goto("/vault");
        return;
    }
    const vault = await unlockVault(password, id);
    const request = session;
    const settings = await vault.loadLockSettings();
    const inactivity = createInactivityLock(() => { void lockVault(); });
    inactivity.start(settings);
    if (request !== session) { vault.close(); return; }
    vaultState = { status: "unlocked", vault, lockSettings: settings, inactivity, isSynced: false };
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
