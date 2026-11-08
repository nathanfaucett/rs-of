import * as v from "valibot";
import { decrypt, encrypt } from "./storage/crypto";

// Consumers:
// - LockSettings.svelte (UI)
// - vault.svelte.ts (state machine, session)
// - routes/(app)/layout/NavSidebar.svelte (lock button)
// - routes/(app)/settings/+page.svelte (lock button)

export const LockSettingsSchema = v.strictObject({
    timeoutMinutes: v.union([v.picklist([1, 5, 15, 30]), v.literal(0)]),
});
export type LockSettings = v.InferOutput<typeof LockSettingsSchema>;
export const DEFAULT_LOCK_SETTINGS: LockSettings = { timeoutMinutes: 5 };

export function isBrowserSession(settings: LockSettings): boolean {
    return settings.timeoutMinutes === 0;
}

const encryptedSchema = v.strictObject({
    ciphertext: v.string(),
    nonce: v.string(),
});
const purpose = (id: string) =>
    new TextEncoder().encode(`password-manager/lock-settings/v1/${id}`);

export const LOCK_TIMEOUT_OPTIONS = [
    { value: 0, label: "Browser session (lock on close)" },
    { value: 1, label: "1 minute" },
    { value: 5, label: "5 minutes" },
    { value: 15, label: "15 minutes" },
    { value: 30, label: "30 minutes" },
] as const;

export async function encryptLockSettings(
    key: CryptoKey,
    id: string,
    settings: LockSettings,
): Promise<string> {
    return JSON.stringify(
        await encrypt(
            key,
            JSON.stringify(v.parse(LockSettingsSchema, settings)),
            purpose(id),
        ),
    );
}

export async function decryptLockSettings(
    key: CryptoKey,
    id: string,
    saved: string | null,
): Promise<LockSettings> {
    if (saved === null) return { ...DEFAULT_LOCK_SETTINGS };
    try {
        const { ciphertext, nonce } = v.parse(
            encryptedSchema,
            JSON.parse(saved),
        );
        return v.parse(
            LockSettingsSchema,
            JSON.parse(await decrypt(key, ciphertext, nonce, purpose(id))),
        );
    } catch {
        throw new Error(
            "Cannot read lock settings. Keep the vault and backup; reopen with a supported app. No settings were reset.",
        );
    }
}

export function createInactivityLock(onlock: () => void, timeProvider: () => number = Date.now) {
    let deadline: number | null = null;
    let duration = 0;
    let browserSession = false;
    return {
        start(settings: LockSettings) {
            browserSession = isBrowserSession(settings);
            if (browserSession) {
                deadline = null;
                duration = 0;
                return;
            }
            duration = v.parse(LockSettingsSchema, settings).timeoutMinutes * 60_000;
            deadline = timeProvider() + duration;
        },
        check() {
            if (browserSession) return false;
            if (deadline === null || timeProvider() < deadline) return false;
            deadline = null;
            onlock();
            return true;
        },
        activity() {
            if (browserSession) return;
            if (this.check() || deadline === null) return;
            deadline = timeProvider() + duration;
        },
        stop() {
            deadline = null;
            browserSession = false;
        },
    };
}
