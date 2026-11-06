import { afterEach, expect, it, vi } from "vitest";
import { createVault, unlockVault } from "$lib/vault/vault";
import { preparePersonalJsonImport } from "./import";
import supported from "../../../../../docs/fixtures/password-manager/supported.json";
import cases from "../../../../../docs/fixtures/password-manager/cases.json";
import { parsePersonalJson } from "./personal-json";

for (const entry of cases.filter((entry) =>
    ["accept", "block"].includes(entry.expect),
)) {
    it(`validates fixture: ${entry.name}`, () => {
        const document = structuredClone(supported) as unknown as Record<
            string,
            unknown
        >;
        for (const patch of entry.patch ?? []) {
            const keys = patch.path.slice(1).split("/");
            const key = keys.pop()!;
            let parent: any = document;
            for (const part of keys) parent = parent[part];
            if (patch.op === "remove") delete parent[key];
            else parent[key] = patch.value;
        }
        const result = parsePersonalJson(entry.raw ?? JSON.stringify(document));
        if (entry.expect === "accept") expect(result.issues).toEqual([]);
        else {
            expect(result.issues.map((issue) => issue.path)).toContain(
                entry.path,
            );
            expect(JSON.stringify(result.issues)).not.toContain(
                "SYNTHETIC-NOT-A-CREDENTIAL",
            );
        }
    });
}

it("previews without writes, commits one complete batch, and requires a repeat choice after reopen", async () => {
    const vault = await createVault(
        "synthetic-import",
        `Import ${crypto.randomUUID()}`,
    );
    const preview = await preparePersonalJsonImport(
        vault,
        JSON.stringify(supported),
    );
    expect(preview.issues).toEqual([]);
    expect(preview.counts).toEqual({
        login: 1,
        secureNote: 1,
        card: 1,
        identity: 1,
        sshKey: 1,
        folders: 2,
    });
    expect((await vault.readItems()).items).toEqual([]);
    await preview.commit();
    const first = await vault.readItems();
    expect(first.items).toHaveLength(5);
    const id = vault.id;
    vault.close();
    const reopened = await unlockVault("synthetic-import", id);
    const repeat = await preparePersonalJsonImport(
        reopened,
        JSON.stringify(supported),
    );
    expect(repeat.duplicates).toEqual({ sourceIds: 5, logical: 5 });
    await expect(repeat.commit()).rejects.toThrow("import-as-new");
    expect(await reopened.readItems()).toEqual(first);
    await repeat.commit("import-as-new");
    const final = await reopened.readItems();
    expect(final.items).toHaveLength(10);
    expect(final.folders).toHaveLength(4);
    expect(new Set(final.items.map((item) => item.id)).size).toBe(10);
    expect(
        first.items.every((item) =>
            final.items.some(
                (value) => JSON.stringify(value) === JSON.stringify(item),
            ),
        ),
    ).toBe(true);
    reopened.close();
});

afterEach(() => vi.restoreAllMocks());

it("requires confirmation for no-ID logical repeats and does not infer duplicates within a file", async () => {
    const vault = await createVault(
        "synthetic",
        `No IDs ${crypto.randomUUID()}`,
    );
    const text = JSON.stringify({
        items: [
            { type: 1, name: "Same", login: {} },
            { type: 1, name: "Same", login: {} },
        ],
    });
    const first = await preparePersonalJsonImport(vault, text);
    expect(first.duplicates).toEqual({ sourceIds: 0, logical: 0 });
    await first.commit();
    const repeat = await preparePersonalJsonImport(vault, text);
    expect(repeat.duplicates).toEqual({ sourceIds: 0, logical: 2 });
    await expect(repeat.commit()).rejects.toThrow("import-as-new");
    repeat.retire();
    await expect(repeat.commit("import-as-new")).rejects.toThrow("closed");
    expect((await vault.readItems()).items).toHaveLength(2);
    vault.close();
});

it("rejects all blocked shared fixtures through the import API without changing the destination", async () => {
    const vault = await createVault(
        "synthetic",
        `Rejected ${crypto.randomUUID()}`,
    );
    const initial = await vault.readItems();
    for (const entry of cases.filter((entry) => entry.expect === "block")) {
        const document = structuredClone(supported);
        for (const patch of entry.patch ?? []) {
            const keys = patch.path.slice(1).split("/");
            const key = keys.pop()!;
            let parent: any = document;
            for (const part of keys) parent = parent[part];
            if (patch.op === "remove") delete parent[key];
            else parent[key] = patch.value;
        }
        const preview = await preparePersonalJsonImport(
            vault,
            entry.raw ?? JSON.stringify(document),
        );
        expect(preview.issues.length).toBeGreaterThan(0);
        await expect(preview.commit("import-as-new")).rejects.toThrow(
            "blocked",
        );
        expect(await vault.readItems()).toEqual(initial);
        preview.retire();
    }
    vault.close();
});

it("enforces size, count, depth, exact types, duplicate escaped members and nested unknown paths", () => {
    for (const text of [
        '{"items":[],"it\\u0065ms":[]}',
        '{"items":[]} trailing',
        '{"items":[{"type":"1","name":"N","login":{}}]}',
        '{"folders":null,"items":[]}',
        JSON.stringify({
            items: Array.from({ length: 10001 }, () => ({
                type: 1,
                name: "N",
                login: {},
            })),
        }),
        " ".repeat(16 * 1024 * 1024 + 1),
        "[".repeat(66) + "0" + "]".repeat(66),
    ])
        expect(parsePersonalJson(text).issues.length).toBeGreaterThan(0);
    const parsed = parsePersonalJson(
        JSON.stringify({
            items: [
                {
                    type: 1,
                    name: "N",
                    login: {
                        uris: [
                            {
                                uri: "scheme-less",
                                match: null,
                                uriChecksum: "PRIVATE-MARKER",
                            },
                        ],
                        future: "PRIVATE-MARKER",
                    },
                    fields: [
                        {
                            name: "N",
                            value: "PRIVATE-MARKER",
                            type: 0,
                            future: 1,
                        },
                    ],
                },
            ],
        }),
    );
    expect(parsed.issues.map((issue) => issue.path)).toEqual([
        "/items/0/fields/0/future",
        "/items/0/login/future",
        "/items/0/login/uris/0/uriChecksum",
    ]);
    expect(JSON.stringify(parsed.issues)).not.toContain("PRIVATE-MARKER");
});

it("preserves submillisecond date instants and normalizes timezone offsets", () => {
    const parsed = parsePersonalJson(
        JSON.stringify({
            items: [
                {
                    type: 2,
                    name: "Precise dates",
                    secureNote: {},
                    creationDate: "2025-01-02T03:04:05.1234567+02:00",
                    revisionDate: "2025-01-02T03:04:05Z",
                    passwordHistory: [
                        {
                            password: "old",
                            lastUsedDate: "2025-01-02T03:04:05.0000001-02:00",
                        },
                    ],
                },
            ],
        }),
    );
    expect(parsed.issues).toEqual([]);
    expect(parsed.items[0].createdAt).toBe("2025-01-02T01:04:05.1234567Z");
    expect(parsed.items[0].updatedAt).toBe("2025-01-02T03:04:05.000Z");
    expect(parsed.items[0].history[0].changedAt).toBe(
        "2025-01-02T05:04:05.0000001Z",
    );
});

it("maps the complete synthetic five-type contract without changing input", () => {
    const input = JSON.stringify(supported);
    const result = parsePersonalJson(input);
    expect(result.issues).toEqual([]);
    expect(result.items.map((item) => item.kind)).toEqual([
        "login",
        "secureNote",
        "card",
        "identity",
        "sshKey",
    ]);
    expect(result.folders.map((folder) => folder.name)).toEqual([
        "Synthetic",
        "Synthetic/日本語",
    ]);
    const login = result.items[0];
    expect(login.kind === "login" && login.secret).toBe(
        "SYNTHETIC-NOT-A-CREDENTIAL",
    );
    expect(
        login.kind === "login" && login.uris.map((uri) => uri.match),
    ).toEqual([
        null,
        "domain",
        "host",
        "startsWith",
        "exact",
        "regex",
        "never",
    ]);
    expect(login.history).toEqual([
        {
            secret: "SYNTHETIC-PRIOR-ONE",
            changedAt: "2025-01-02T03:04:05.000Z",
        },
        { secret: "", changedAt: "2025-02-03T04:05:06.000Z" },
    ]);
    expect(login.fields.map((field) => field.type)).toEqual([
        "text",
        "hidden",
        "boolean",
        "boolean",
        "text",
        "text",
    ]);
    expect(login.folderId).toBe(result.folders[1].id);
    expect(login.notes).toBe("Synthetic only.\nSecond line, café.\n");
    expect(login.favorite).toBe(true);
    expect(login.createdAt).toBe("2025-01-01T00:00:00.000Z");
    expect(login.updatedAt).toBe("2025-03-01T00:00:00.000Z");
    expect(login.kind === "login" && login.username).toBe(
        "synthetic@example.invalid",
    );
    expect(login.kind === "login" && login.totp).toBe("JBSWY3DPEHPK3PXP");
    expect(result.items[1]).toMatchObject({
        kind: "secureNote",
        type: 0,
        notes: "Private text fixture only.\nNo password field.\n",
        folderId: null,
    });
    expect(result.items[2]).toMatchObject({
        kind: "card",
        ...supported.items[2].card,
        notes: null,
        folderId: result.folders[0].id,
    });
    expect(result.items[3]).toMatchObject({
        kind: "identity",
        ...supported.items[3].identity,
    });
    expect(result.items[4]).toMatchObject({
        kind: "sshKey",
        ...supported.items[4].sshKey,
    });
    expect(login.id).not.toBe(supported.items[0].id);
    expect(login.importSource?.id).toBe(supported.items[0].id);
    expect(JSON.stringify(supported)).toBe(input);
});
