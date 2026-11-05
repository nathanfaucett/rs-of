import * as v from "valibot";
import { describe, expect, it } from "vitest";
import { LoginSchema } from "./secret";
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
