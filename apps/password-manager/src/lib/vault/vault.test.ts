import { expect, it } from "vitest";
import { createVault, unlockVault } from "./vault";

it("persists encrypted logins across lock/unlock and permanently removes a deleted login", async () => {
    const name = `test-crud-${crypto.randomUUID()}`;
    const password = "test-only-master-password";
    const plaintext = JSON.stringify({
        name: "Test login",
        username: "test-user",
        secret: "test-password",
        uris: [
            { uri: "https://example.com", match: "domain" },
            { uri: "https://example.org", match: "exact" },
        ],
    });
    const vault = await createVault(password, name);
    let id: string;
    try {
        const stored = await vault.encryptAndStore(plaintext);
        id = stored.id;
        expect(stored.ciphertext).not.toContain("test-password");
        expect(await vault.decryptSecret((await vault.loadSecrets())[0])).toBe(
            plaintext,
        );
    } finally {
        vault.close();
    }
    const unlocked = await unlockVault(password, name);
    try {
        expect(
            await unlocked.decryptSecret((await unlocked.loadSecrets())[0]),
        ).toBe(plaintext);
        await unlocked.deleteSecret(id);
        expect(await unlocked.loadSecrets()).toEqual([]);
    } finally {
        unlocked.close();
    }
    const reopened = await unlockVault(password, name);
    try {
        expect(await reopened.loadSecrets()).toEqual([]);
    } finally {
        reopened.close();
    }
}, 15000);
