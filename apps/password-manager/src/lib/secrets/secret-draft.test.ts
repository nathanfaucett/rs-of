import * as v from "valibot";
import { describe, expect, it } from "vitest";
import { LoginSchema } from "./secret";
import { serializeJsonExport } from "../json-export/export";
import { constructSecret, createSecretDraft } from "./secret-draft";

const id = "12345678-1234-4234-8234-123456789abc";
const createdAt = "2026-01-01T00:00:00.000Z";
const now = "2026-11-04T00:00:00.000Z";
const record = v.parse(LoginSchema, {
    kind: "login",
    id,
    name: "Login",
    secret: "old password",
    createdAt,
    updatedAt: createdAt,
    uris: [
        { uri: "https://example.com", match: "host" },
        { uri: "https://example.org", match: "exact" },
    ],
    fields: [{ name: "PIN", value: "1234", type: "hidden" }],
});

describe("secret drafts", () => {
    it("loads existing records without new optional login fields", () => {
        const draft = createSecretDraft(record);
        expect(draft.username).toBe("");
        expect(draft.totp).toBe("");
        expect(draft.uris).toEqual(record.uris);
    });

    it("keeps unsaved URL and custom field edits separate", () => {
        const draft = createSecretDraft(record);
        draft.uris[0].uri = "changed";
        draft.fields[0].value = "changed";
        draft.uris.pop();
        expect(record.uris).toHaveLength(2);
        expect(record.uris[0].uri).toBe("https://example.com");
        expect(record.fields[0].value).toBe("1234");
    });

    it("saves all login fields and URLs, removes blank URLs, and retains history", () => {
        const draft = createSecretDraft(record);
        draft.username = "user@example.com";
        draft.totp = "test-authenticator-key";
        draft.secret = "new password";
        draft.uris.push({ uri: "  ", match: "domain" });
        draft.fields.push({ name: "Enabled", value: "false", type: "boolean" });
        draft.notes = "Login notes";
        draft.favorite = true;
        const result = constructSecret(record, draft, id, now);
        expect(result.success).toBe(true);
        if (!result.success) throw new Error("Expected a valid login");
        expect(result.output).toMatchObject({
            username: draft.username,
            totp: draft.totp,
            secret: draft.secret,
            uris: record.uris,
            fields: draft.fields,
            notes: draft.notes,
            favorite: true,
            createdAt,
            updatedAt: now,
            history: [{ secret: record.secret, changedAt: now }],
        });
        const unchanged = constructSecret(
            result.output,
            createSecretDraft(result.output),
            id,
            now,
        );
        expect(unchanged.success && unchanged.output.history).toEqual(
            result.output.history,
        );
    });

    it.each([null, "", "TRUE", "0"])(
        "rejects a new or changed Boolean %s at construction, but preserves an unchanged legacy field",
        (value) => {
            const draft = createSecretDraft(record);
            draft.fields.push({ name: "Flag", type: "boolean", value });
            expect(constructSecret(record, draft, id, now).success).toBe(false);
            const legacy = {
                ...record,
                fields: [{ name: "Flag", type: "boolean" as const, value }],
            };
            const unchanged = constructSecret(
                legacy,
                createSecretDraft(legacy),
                id,
                now,
            );
            expect(unchanged.success && unchanged.output.fields).toEqual(
                legacy.fields,
            );
            const valid = {
                ...legacy,
                fields: [
                    { name: "Flag", type: "boolean" as const, value: "true" },
                ],
            };
            const changed = createSecretDraft(valid);
            changed.fields[0].value = value;
            expect(constructSecret(valid, changed, id, now).success).toBe(
                false,
            );
        },
    );

    it("preserves an unchanged legacy Boolean after removing a preceding Text field", () => {
        const legacy = {
            ...record,
            fields: [
                { name: "Text", type: "text" as const, value: "Unrelated" },
                { name: "Legacy", type: "boolean" as const, value: null },
            ],
        };
        const draft = createSecretDraft(legacy);
        draft.fields.splice(0, 1);
        const result = constructSecret(legacy, draft, id, now);
        expect(result.success).toBe(true);
        if (!result.success)
            throw new Error("Expected the unchanged legacy field to save");
        expect(result.output.fields).toEqual([legacy.fields[1]]);
        expect(Reflect.ownKeys(result.output.fields[0]).sort()).toEqual([
            "name",
            "type",
            "value",
        ]);
        expect(() =>
            serializeJsonExport({ items: [result.output], folders: [] }),
        ).toThrow("Cannot represent Boolean at /items/0/fields/0/value");
    });

    it.each(["replacement", "copy", "same origin", "altered"] as const)(
        "rejects an invalid legacy Boolean %s rather than treating it as an unchanged field",
        (operation) => {
            const field = {
                name: "Legacy",
                type: "boolean" as const,
                value: null,
            };
            const legacy = { ...record, fields: [field, { ...field }] };
            expect(
                constructSecret(legacy, createSecretDraft(legacy), id, now)
                    .success,
            ).toBe(true);
            const draft = createSecretDraft(legacy);
            if (operation === "replacement") draft.fields[0] = { ...field };
            else if (operation === "copy")
                draft.fields[1] = { ...draft.fields[0] };
            else if (operation === "same origin")
                draft.fields[1] = draft.fields[0];
            else draft.fields[0].name = "Changed";
            expect(constructSecret(legacy, draft, id, now).success).toBe(false);
        },
    );

    it("rejects missing names and invalid match modes but accepts absent passwords", () => {
        const draft = createSecretDraft(null);
        expect(constructSecret(null, draft, id, now).success).toBe(false);
        draft.name = "Login";
        expect(constructSecret(null, draft, id, now).success).toBe(true);
        draft.secret = "password";
        expect(constructSecret(null, draft, id, now).success).toBe(true);
        const invalid = {
            ...draft,
            uris: [{ uri: "https://example.com", match: "invalid" }],
        };
        expect(
            constructSecret(null, invalid as typeof draft, id, now).success,
        ).toBe(false);
    });
});
