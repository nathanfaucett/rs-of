import { Database } from "@ofsdk/sql";
import { expect, it, vi } from "vitest";
import {
    fixturePassword,
    fixturePlaintext,
    legacyVault,
} from "./authentication-fixtures";
import { vaultMigrations } from "./migrations";
import { createVault, unlockVault } from "./vault";

async function storage(name: string) {
    const db = await Database.open(`password-manager-vault-${name}`);
    const client = db.client();
    return {
        client,
        close() {
            client.close();
            db.close();
        },
    };
}

it("rejects wrong passwords without changing populated or last-item-removed vaults", async () => {
    const name = `test-auth-${crypto.randomUUID()}`;
    const vault = await createVault(fixturePassword, name);
    const record = await vault.encryptAndStore(fixturePlaintext);
    vault.close();
    for (const populated of [true, false]) {
        const db = await storage(name);
        const before = await db.client.execute(
            "SELECT * FROM vault_meta; SELECT * FROM secrets",
        );
        await expect(unlockVault("wrong-password", name)).rejects.toThrow(
            "Wrong password",
        );
        expect(
            await db.client.execute(
                "SELECT * FROM vault_meta; SELECT * FROM secrets",
            ),
        ).toEqual(before);
        db.close();
        const reopened = await unlockVault(fixturePassword, name);
        try {
            const records = await reopened.loadSecrets();
            if (populated) {
                expect(records).toEqual([record]);
                expect(await reopened.decryptSecret(records[0])).toBe(
                    fixturePlaintext,
                );
                await reopened.deleteSecret(record.id);
            } else {
                expect(records).toEqual([]);
            }
        } finally {
            reopened.close();
        }
    }
}, 15000);

it("upgrades v1 only after password proof and preserves all source records", async () => {
    const name = `test-legacy-${crypto.randomUUID()}`;
    await legacyVault(name, true);
    const db = await storage(name);
    try {
        const before = await db.client.execute(
            "SELECT vault_id, salt, iv, created_at FROM vault_meta; SELECT * FROM secrets",
        );
        await expect(unlockVault("wrong-password", name)).rejects.toThrow(
            "Wrong password",
        );
        expect(
            await db.client.execute("SELECT authentication FROM vault_meta"),
        ).toMatchObject([{ rows: [[{ type: "null" }]] }]);
        const upgraded = await unlockVault(fixturePassword, name);
        try {
            const [record] = await upgraded.loadSecrets();
            expect(await upgraded.decryptSecret(record)).toBe(fixturePlaintext);
            expect(
                await db.client.execute(
                    "SELECT vault_id, salt, iv, created_at FROM vault_meta; SELECT * FROM secrets",
                ),
            ).toEqual(before);
            await upgraded.deleteSecret(record.id);
        } finally {
            upgraded.close();
        }
        await expect(unlockVault("wrong-password", name)).rejects.toThrow(
            "Wrong password",
        );
        const reopened = await unlockVault(fixturePassword, name);
        expect(await reopened.loadSecrets()).toEqual([]);
        reopened.close();
    } finally {
        db.close();
    }
}, 15000);

it("keeps a legacy empty vault unchanged without setting any attempted password", async () => {
    const name = `test-legacy-empty-${crypto.randomUUID()}`;
    await legacyVault(name, false);
    const db = await storage(name);
    try {
        const before = await db.client.execute(
            "SELECT * FROM vault_meta; SELECT * FROM secrets",
        );
        for (const password of ["wrong-password", fixturePassword]) {
            await expect(unlockVault(password, name)).rejects.toThrow(
                "original password cannot be verified",
            );
        }
        const after = await db.client.execute(
            "SELECT * FROM vault_meta; SELECT * FROM secrets",
        );
        expect(after[0].rows[0].slice(0, 5)).toEqual(before[0].rows[0]);
        expect(after[0].rows[0][5]).toEqual({ type: "null" });
        expect(after[1]).toEqual(before[1]);
    } finally {
        db.close();
    }
}, 15000);

it("fails closed on malformed, unsupported, truncated, tampered, and mismatched authentication", async () => {
    const name = `test-proof-${crypto.randomUUID()}`;
    const vault = await createVault(fixturePassword, name);
    await vault.encryptAndStore(fixturePlaintext);
    vault.close();
    const db = await storage(name);
    try {
        const records = await db.client.execute("SELECT * FROM secrets");
        const original = await db.client.execute(
            "SELECT authentication FROM vault_meta",
        );
        const cell = original[0].rows[0][0];
        if (cell.type !== "text") throw new Error("Missing fixture proof");
        expect(cell.value).not.toContain(fixturePassword);
        expect(cell.value).not.toContain(fixturePlaintext);
        const proof = JSON.parse(cell.value);
        for (const damaged of [
            "not-json",
            "null",
            "{}",
            JSON.stringify({ ...proof, version: 2 }),
            JSON.stringify({ ...proof, nonce: "!" }),
            JSON.stringify({ ...proof, ciphertext: "AA==" }),
            JSON.stringify({
                ...proof,
                ciphertext: `AAAA${proof.ciphertext.slice(4)}`,
            }),
        ]) {
            await db.client.execute(
                "UPDATE vault_meta SET authentication = $1",
                [{ type: "text", value: damaged }],
            );
            await expect(unlockVault(fixturePassword, name)).rejects.toThrow(
                "damaged vault authentication",
            );
            expect(
                await db.client.execute(
                    "SELECT authentication FROM vault_meta",
                ),
            ).toMatchObject([{ rows: [[{ type: "text", value: damaged }]] }]);
        }
        await db.client.execute(
            "UPDATE vault_meta SET authentication = $1, created_at = 'changed'",
            [cell],
        );
        await expect(unlockVault(fixturePassword, name)).rejects.toThrow(
            "damaged vault authentication",
        );
        expect(await db.client.execute("SELECT * FROM secrets")).toEqual(
            records,
        );
    } finally {
        db.close();
    }
}, 15000);

it("does not upgrade a legacy vault if any encrypted record is damaged", async () => {
    const name = `test-legacy-corrupt-${crypto.randomUUID()}`;
    await legacyVault(name, true);
    const db = await storage(name);
    try {
        await db.client.execute(
            "INSERT INTO secrets (id, ciphertext, nonce, created_at, updated_at) VALUES ('damaged', 'AA==', 'AA==', 'old', 'old')",
        );
        const before = await db.client.execute("SELECT * FROM secrets");
        await expect(unlockVault(fixturePassword, name)).rejects.toThrow(
            "damaged legacy records",
        );
        expect(await db.client.execute("SELECT * FROM secrets")).toEqual(
            before,
        );
        expect(
            await db.client.execute("SELECT authentication FROM vault_meta"),
        ).toMatchObject([{ rows: [[{ type: "null" }]] }]);
    } finally {
        db.close();
    }
}, 15000);

it("does not return an unlocked handle when creation cannot persist authentication", async () => {
    const name = `test-write-failure-${crypto.randomUUID()}`;
    const db = await storage(name);
    try {
        await db.client.migrate(vaultMigrations);
        const failure = vi
            .spyOn(IDBObjectStore.prototype, "put")
            .mockImplementation(() => {
                throw new DOMException(
                    "Synthetic storage failure",
                    "QuotaExceededError",
                );
            });
        try {
            await expect(createVault(fixturePassword, name)).rejects.toThrow(
                "Could not confirm vault creation",
            );
            expect(failure).toHaveBeenCalled();
        } finally {
            failure.mockRestore();
        }
        expect(
            (await db.client.execute("SELECT * FROM vault_meta"))[0].rows,
        ).toEqual([]);
        const retried = await createVault(fixturePassword, name);
        retried.close();
        await expect(unlockVault("wrong-password", name)).rejects.toThrow(
            "Wrong password",
        );
        const reopened = await unlockVault(fixturePassword, name);
        reopened.close();
    } finally {
        db.close();
    }
}, 15000);

it("leaves legacy records usable after an authentication upgrade write fails", async () => {
    const name = `test-upgrade-failure-${crypto.randomUUID()}`;
    await legacyVault(name, true);
    const db = await storage(name);
    try {
        await db.client.migrate(vaultMigrations);
        const before = await db.client.execute(
            "SELECT * FROM vault_meta; SELECT * FROM secrets",
        );
        const failure = vi
            .spyOn(IDBObjectStore.prototype, "put")
            .mockImplementation(() => {
                throw new DOMException(
                    "Synthetic storage failure",
                    "QuotaExceededError",
                );
            });
        try {
            await expect(unlockVault(fixturePassword, name)).rejects.toThrow(
                "Could not confirm the vault authentication upgrade",
            );
            expect(failure).toHaveBeenCalled();
        } finally {
            failure.mockRestore();
        }
        expect(
            await db.client.execute(
                "SELECT * FROM vault_meta; SELECT * FROM secrets",
            ),
        ).toEqual(before);
        const reopened = await unlockVault(fixturePassword, name);
        try {
            expect(
                await reopened.decryptSecret((await reopened.loadSecrets())[0]),
            ).toBe(fixturePlaintext);
        } finally {
            reopened.close();
        }
    } finally {
        db.close();
    }
}, 15000);
