import * as v from "valibot";
import { decrypt, encrypt } from "./crypto";

export const LockSettingsSchema = v.strictObject({
    timeoutMinutes: v.picklist([1, 5, 15, 30]),
});
export type LockSettings = v.InferOutput<typeof LockSettingsSchema>;
export const DEFAULT_LOCK_SETTINGS: LockSettings = { timeoutMinutes: 5 };

const encryptedSchema = v.strictObject({
    ciphertext: v.string(),
    nonce: v.string(),
});
const purpose = (id: string) =>
    new TextEncoder().encode(`password-manager/lock-settings/v1/${id}`);

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

export function createInactivityLock(onlock: () => void) {
    let deadline: number | null = null;
    let duration = 0;
    return {
        start(settings: LockSettings) {
            duration =
                v.parse(LockSettingsSchema, settings).timeoutMinutes * 60_000;
            deadline = Date.now() + duration;
        },
        check() {
            if (deadline === null || Date.now() < deadline) return false;
            deadline = null;
            onlock();
            return true;
        },
        activity() {
            if (this.check() || deadline === null) return;
            deadline = Date.now() + duration;
        },
        stop() {
            deadline = null;
        },
    };
}
