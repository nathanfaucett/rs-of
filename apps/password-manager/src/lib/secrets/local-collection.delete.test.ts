// @vitest-environment playwright
import { afterEach, describe, expect, it, vi } from "vitest";
import type { EncryptedSecret, VaultHandle } from "../vault/vault";
import {
    createLocalSecretCollection,
    UnacknowledgedStorageWriteError,
} from "./local-collection";
import type { Login as Secret } from "./secret";

const secret: Secret = {
    kind: "login",
    id: "12345678-1234-4234-8234-123456789abc",
    name: "Delete me",
    secret: "password",
    uris: [],
    favorite: false,
    fields: [],
    history: [],
    createdAt: "2026-01-01T00:00:00.000Z",
    updatedAt: "2026-01-01T00:00:00.000Z",
};
const other: Secret = {
    ...secret,
    id: "12345678-1234-4234-8234-123456789abd",
    name: "Keep me",
};
const collections: ReturnType<typeof createLocalSecretCollection>[] = [];

afterEach(async () => {
    await Promise.all(
        collections.splice(0).map((collection) => collection.cleanup()),
    );
});

async function setup(remove: boolean) {
    const records = new Map(
        [secret, other].map((record) => [record.id, record]),
    );
    const vault = {
        id: "deletion-test",
        prepareItems: vi.fn(async () => {}),
        readItems: vi.fn(async () => {
            const items = [];
            for (const record of await vault.loadSecrets())
                items.push(JSON.parse(await vault.decryptSecret(record)));
            return { items, folders: [] };
        }),
        loadSecrets: vi.fn(async () =>
            [...records.values()].map(
                (record): EncryptedSecret => ({
                    id: record.id,
                    ciphertext: JSON.stringify(record),
                    nonce: "test",
                    createdAt: record.createdAt,
                    updatedAt: record.updatedAt,
                }),
            ),
        ),
        decryptSecret: vi.fn(
            async (record: EncryptedSecret) => record.ciphertext,
        ),
        encryptAndStore: vi.fn(async (plaintext: string) => {
            const record: Secret = JSON.parse(plaintext);
            records.set(record.id, record);
        }),
        deleteSecret: vi.fn(async (id: string) => {
            if (remove) records.delete(id);
        }),
    };
    const collection = createLocalSecretCollection({
        vault: vault as unknown as VaultHandle,
    });
    collections.push(collection);
    await collection.preload();
    return { collection, vault, records };
}

describe("local secret writes", () => {
    it("rejects an edit when storage returns the old record, then permits a retry", async () => {
        const { collection, vault, records } = await setup(true);
        vault.encryptAndStore.mockImplementationOnce(async () => undefined);
        await expect(
            collection.update(secret.id, (record) => {
                if (record.kind === "login") record.secret = "changed password";
            }).isPersisted.promise,
        ).rejects.toBeInstanceOf(UnacknowledgedStorageWriteError);
        expect((collection.get(secret.id) as Secret | undefined)?.secret).toBe(
            secret.secret,
        );
        expect(records.get(secret.id)?.secret).toBe(secret.secret);
        await collection.update(secret.id, (record) => {
            if (record.kind === "login") record.secret = "changed password";
        }).isPersisted.promise;
        await collection.utils.refresh();
        expect((collection.get(secret.id) as Secret | undefined)?.secret).toBe(
            "changed password",
        );
        expect(collection.get(other.id)).toMatchObject(other);
    });
    it("removes only the selected secret and stays deleted after refresh", async () => {
        const { collection, vault, records } = await setup(true);

        await collection.delete(secret.id).isPersisted.promise;
        expect(vault.deleteSecret).toHaveBeenCalledExactlyOnceWith(
            secret.id,
            secret,
        );
        expect(records.has(secret.id)).toBe(false);
        expect(collection.has(secret.id)).toBe(false);
        expect(collection.get(other.id)).toMatchObject(other);

        await collection.utils.refresh();
        expect(collection.has(secret.id)).toBe(false);
        expect(collection.get(other.id)).toMatchObject(other);
    });

    it("rejects a deletion when storage still contains the secret", async () => {
        const { collection, vault, records } = await setup(false);

        await expect(
            collection.delete(secret.id).isPersisted.promise,
        ).rejects.toBeInstanceOf(UnacknowledgedStorageWriteError);
        expect(collection.get(secret.id)).toMatchObject(secret);
        expect(collection.get(other.id)).toMatchObject(other);
        expect(collection.status).toBe("ready");

        await collection.utils.refresh();
        expect(collection.get(secret.id)).toMatchObject(secret);

        vault.deleteSecret.mockImplementationOnce(async (id) => {
            records.delete(id);
        });
        await collection.delete(secret.id).isPersisted.promise;
        expect(collection.has(secret.id)).toBe(false);
        expect(collection.get(other.id)).toMatchObject(other);
    });

    it("rejects storage errors without losing either secret", async () => {
        const { collection, vault } = await setup(true);
        vault.deleteSecret.mockRejectedValueOnce(new Error("Delete failed"));

        await expect(
            collection.delete(secret.id).isPersisted.promise,
        ).rejects.toThrow("Delete failed");
        expect(collection.get(secret.id)).toMatchObject(secret);
        expect(collection.get(other.id)).toMatchObject(other);

        await collection.utils.refresh();
        await collection.delete(secret.id).isPersisted.promise;
        expect(collection.has(secret.id)).toBe(false);
        expect(collection.get(other.id)).toMatchObject(other);
    });

    it("reports an unknown outcome if the deletion cannot be reloaded", async () => {
        const { collection, vault, records } = await setup(true);
        vault.loadSecrets.mockRejectedValueOnce(new Error("Read failed"));

        await expect(
            collection.delete(secret.id).isPersisted.promise,
        ).rejects.toThrow("Write outcome could not be reconciled");
        expect(records.has(secret.id)).toBe(false);

        await collection.utils.refresh();
        expect(collection.has(secret.id)).toBe(false);
        expect(collection.get(other.id)).toMatchObject(other);
    });
});
