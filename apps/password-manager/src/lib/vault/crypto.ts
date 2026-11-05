import type { VaultMeta } from "./vault";

export const PBKDF2_ITERATIONS = 600_000;

export function toBase64(buffer: ArrayBuffer): string {
    const bytes = new Uint8Array(buffer);
    let binary = "";
    for (let i = 0; i < bytes.byteLength; i++)
        binary += String.fromCharCode(bytes[i]);
    return btoa(binary);
}

export function fromBase64(base64: string): ArrayBuffer {
    const binary = atob(base64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
    return bytes.buffer;
}

export async function deriveKey(
    password: string,
    salt: ArrayBuffer,
): Promise<CryptoKey> {
    const material = await crypto.subtle.importKey(
        "raw",
        new TextEncoder().encode(password),
        "PBKDF2",
        false,
        ["deriveKey"],
    );
    return crypto.subtle.deriveKey(
        {
            name: "PBKDF2",
            salt,
            iterations: PBKDF2_ITERATIONS,
            hash: "SHA-512",
        },
        material,
        { name: "AES-GCM", length: 256 },
        false,
        ["encrypt", "decrypt"],
    );
}

export async function encrypt(
    key: CryptoKey,
    plaintext: string,
    additionalData?: Uint8Array<ArrayBuffer>,
): Promise<{ ciphertext: string; nonce: string }> {
    const nonce = crypto.getRandomValues(new Uint8Array(12));
    const encrypted = await crypto.subtle.encrypt(
        {
            name: "AES-GCM",
            iv: nonce,
            ...(additionalData ? { additionalData } : {}),
        },
        key,
        new TextEncoder().encode(plaintext),
    );
    return { ciphertext: toBase64(encrypted), nonce: toBase64(nonce.buffer) };
}

export async function decrypt(
    key: CryptoKey,
    ciphertext: string,
    nonce: string,
    additionalData?: Uint8Array<ArrayBuffer>,
): Promise<string> {
    const decrypted = await crypto.subtle.decrypt(
        {
            name: "AES-GCM",
            iv: fromBase64(nonce),
            ...(additionalData ? { additionalData } : {}),
        },
        key,
        fromBase64(ciphertext),
    );
    return new TextDecoder("utf-8", { fatal: true }).decode(decrypted);
}

function authenticationPlaintext(meta: VaultMeta): string {
    return JSON.stringify([
        "password-manager/vault-authentication",
        1,
        meta.id,
        meta.salt,
        meta.iv,
        meta.createdAt,
    ]);
}

export async function createAuthentication(
    key: CryptoKey,
    meta: VaultMeta,
): Promise<string> {
    return JSON.stringify({
        version: 1,
        ...(await encrypt(key, authenticationPlaintext(meta))),
    });
}

export async function verifyAuthentication(
    key: CryptoKey,
    meta: VaultMeta,
    authentication: string,
): Promise<void> {
    try {
        const proof = JSON.parse(authentication);
        if (
            proof?.version !== 1 ||
            typeof proof.ciphertext !== "string" ||
            typeof proof.nonce !== "string" ||
            fromBase64(proof.nonce).byteLength !== 12 ||
            (await decrypt(key, proof.ciphertext, proof.nonce)) !==
                authenticationPlaintext(meta)
        ) {
            throw new Error("Invalid authentication proof");
        }
    } catch {
        throw new Error(
            "Wrong password or damaged vault authentication. Retry with the correct password. If this continues, keep the vault and recover from a backup.",
        );
    }
}
