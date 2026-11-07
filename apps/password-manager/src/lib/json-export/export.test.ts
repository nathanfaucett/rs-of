// @vitest-environment playwright
import * as v from "valibot";
import { expect, it } from "vitest";
import expected from "$fixtures/password-manager/export-expected.json";
import local from "$fixtures/password-manager/export-local.json";
import { SecretSchema } from "../secrets/secret";
import { parsePersonalJson } from "../import/personal-json";
import type { ItemSnapshot } from "../vault/folder";
import { inspectJsonExport, serializeJsonExport } from "./export";

function snapshot(): ItemSnapshot {
    return {
        items: v.parse(v.array(SecretSchema), structuredClone(local.items)),
        folders: structuredClone(local.folders),
    };
}
it("serializes known local logical values to the independent reference fixture", () => {
    expect(JSON.parse(serializeJsonExport(snapshot()))).toEqual(expected);
});
it.each([
    ["text", null],
    ["text", ""],
    ["hidden", null],
    ["hidden", ""],
    ["boolean", "true"],
    ["boolean", "false"],
])(
    "preserves %s custom-field value %s and history on non-login items",
    (type, value) => {
        const input = snapshot();
        input.items = [
            v.parse(SecretSchema, {
                ...input.items[1],
                fields: [{ name: "same", type, value }],
                history: [
                    { secret: "", changedAt: "2025-01-01T01:00:00+01:00" },
                ],
            }),
        ];
        const result = JSON.parse(serializeJsonExport(input)).items[0];
        expect(result.fields).toEqual([
            {
                name: "same",
                type: type === "text" ? 0 : type === "hidden" ? 1 : 2,
                value,
            },
        ]);
        expect(result.passwordHistory).toEqual([
            { password: "", lastUsedDate: "2025-01-01T00:00:00.000Z" },
        ]);
    },
);
it.each([null, "", "TRUE", "0", "SYNTHETIC-PRIVATE-VALUE"])(
    "blocks legacy unrepresentable Boolean %s without exposing its value",
    (value) => {
        const input = snapshot();
        input.items[0].fields[2].value = value;
        expect(() => serializeJsonExport(input)).toThrow(
            "/items/0/fields/2/value",
        );
        try {
            serializeJsonExport(input);
        } catch (cause) {
            expect(String(cause)).not.toContain("SYNTHETIC-PRIVATE-VALUE");
        }
    },
);
it.each([
    (input: ItemSnapshot) =>
        Object.assign(input.items[0], { future: "SYNTHETIC-PRIVATE-VALUE" }),
    (input: ItemSnapshot) =>
        Object.assign(input.items[0], {
            uris: [{ uri: "synthetic", future: "SYNTHETIC-PRIVATE-VALUE" }],
        }),
    (input: ItemSnapshot) =>
        Object.assign(input.items[0].fields[0], { type: "linked" }),
    (input: ItemSnapshot) =>
        Object.assign(input.items[0], {
            uris: [{ uri: "synthetic", match: "new-rule" }],
        }),
    (input: ItemSnapshot) =>
        Object.assign(input.items[0], { kind: "future-kind" }),
    (input: ItemSnapshot) =>
        Object.assign(input.items[0].history[0], {
            extra: "SYNTHETIC-PRIVATE-VALUE",
        }),
    (input: ItemSnapshot) =>
        Object.assign(input.folders[0], {
            parentId: "SYNTHETIC-PRIVATE-VALUE",
        }),
    (input: ItemSnapshot) =>
        Object.assign(input, { extra: "SYNTHETIC-PRIVATE-VALUE" }),
])("blocks unknown keys and variants before serialization", (mutate) => {
    const input = snapshot();
    mutate(input);
    expect(() => inspectJsonExport(input)).toThrow("Export blocked");
    try {
        inspectJsonExport(input);
    } catch (cause) {
        expect(String(cause)).not.toContain("SYNTHETIC-PRIVATE-VALUE");
    }
});
it.each([
    "2025-02-30T00:00:00Z",
    "2025-01-01T24:00:00Z",
    "2025-01-01T00:60:00Z",
    "2025-01-01T00:00:60Z",
    "2025-01-01T00:00:00+24:00",
    "2025-01-01T00:00:00",
])("rejects invalid timestamp %s on import and export", (value) => {
    const wire = structuredClone(expected);
    wire.items[0].creationDate = value;
    expect(parsePersonalJson(JSON.stringify(wire)).issues).not.toEqual([]);
    const input = snapshot();
    input.items[0].createdAt = value;
    expect(() => serializeJsonExport(input)).toThrow(
        /Cannot represent date|Unsupported or invalid data/,
    );
});
it.each([
    ["2025-01-01T01:02:03.1234567+02:00", "2024-12-31T23:02:03.1234567Z"],
    ["2025-01-01T01:02:03.0001000-03:30", "2025-01-01T04:32:03.0001Z"],
])(
    "round-trips imported fractional timestamp %s without precision loss",
    (source, normalized) => {
        const wire = structuredClone(expected);
        wire.items[0].creationDate = source;
        wire.items[0].revisionDate = source;
        wire.items[0].passwordHistory[0].lastUsedDate = source;
        const imported = parsePersonalJson(JSON.stringify(wire));
        expect(imported.issues).toEqual([]);
        const exported = serializeJsonExport({
            items: imported.items,
            folders: imported.folders,
        });
        const emitted = JSON.parse(exported).items[0];
        expect(emitted.creationDate).toBe(normalized);
        expect(emitted.revisionDate).toBe(normalized);
        expect(emitted.passwordHistory[0].lastUsedDate).toBe(normalized);
        const reimported = parsePersonalJson(exported);
        expect(reimported.issues).toEqual([]);
        expect(reimported.items[0].createdAt).toBe(normalized);
        expect(reimported.items[0].updatedAt).toBe(normalized);
        expect(reimported.items[0].history[0].changedAt).toBe(normalized);
    },
);

it("rejects dangling memberships and duplicate identities", () => {
    const input = snapshot();
    input.folders = [];
    expect(() => serializeJsonExport(input)).toThrow(
        "Malformed vault snapshot",
    );
    const duplicate = snapshot();
    duplicate.items.push(duplicate.items[0]);
    expect(() => serializeJsonExport(duplicate)).toThrow(
        "Malformed vault snapshot",
    );
});
it.each(["login", "secureNote", "card", "identity", "sshKey"])(
    "emits null optional scalars and empty arrays for minimal %s",
    (kind) => {
        const item = v.parse(SecretSchema, {
            kind,
            id: "20000000-0000-4000-8000-000000000001",
            name: "Minimal",
            createdAt: "2025-01-01T00:00:00Z",
            updatedAt: "2025-01-01T00:00:00Z",
            ...(kind === "sshKey" ? { privateKey: "SYNTHETIC" } : {}),
        });
        const output = JSON.parse(
            serializeJsonExport({ items: [item], folders: [] }),
        ).items[0];
        expect(output.notes).toBeNull();
        expect(output.folderId).toBeNull();
        expect(output.favorite).toBe(false);
        expect(output.fields).toEqual([]);
        expect(output.passwordHistory).toEqual([]);
        if (kind === "login")
            expect(output.login).toEqual({
                username: null,
                password: null,
                totp: null,
                uris: [],
                fido2Credentials: [],
            });
        if (kind === "secureNote")
            expect(output.secureNote).toEqual({ type: 0 });
        if (kind === "card")
            expect(output.card).toEqual({
                cardholderName: null,
                brand: null,
                number: null,
                expMonth: null,
                expYear: null,
                code: null,
            });
        if (kind === "identity")
            expect(Object.values(output.identity)).toEqual(
                Array(18).fill(null),
            );
        if (kind === "sshKey")
            expect(output.sshKey).toEqual({
                privateKey: "SYNTHETIC",
                publicKey: null,
                keyFingerprint: null,
            });
    },
);
