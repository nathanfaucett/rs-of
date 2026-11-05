import * as v from "valibot";
import { identityFields } from "./identity-fields";
import {
    type Card,
    CardSchema,
    CustomFieldSchema,
    type Secret,
    type Identity,
    IdentitySchema,
    type Login,
    LoginSchema,
    type SecureNote,
    SecureNoteSchema,
    type SshKey,
    SshKeySchema,
} from "./secret";

export type EditableSecret = Login | SecureNote | Card | Identity | SshKey;
export type EditableKind = EditableSecret["kind"];
type CommonDraft = Pick<Login, "name" | "fields" | "favorite"> & {
    notes: string;
};
export type LoginDraft = CommonDraft &
    Pick<Login, "kind" | "uris"> & {
        secret: string;
        username: string;
        totp: string;
    };
export type SecureNoteDraft = CommonDraft & Pick<SecureNote, "kind" | "type">;
const cardKeys = [
    "cardholderName",
    "brand",
    "number",
    "expMonth",
    "expYear",
    "code",
] as const;
export type CardDraft = CommonDraft & { kind: "card" } & Record<
        (typeof cardKeys)[number],
        string
    >;
export type IdentityDraft = CommonDraft &
    Pick<Identity, "kind"> &
    Record<(typeof identityFields)[number][0], string>;
export type SshKeyDraft = CommonDraft &
    Pick<SshKey, "kind" | "privateKey"> & {
        publicKey: string;
        keyFingerprint: string;
    };
export type SecretDraft =
    | LoginDraft
    | SecureNoteDraft
    | CardDraft
    | IdentityDraft
    | SshKeyDraft;

export function createSecretDraft(record: Login | null): LoginDraft;
export function createSecretDraft(record: SecureNote): SecureNoteDraft;
export function createSecretDraft(record: Card): CardDraft;
export function createSecretDraft(record: Identity): IdentityDraft;
export function createSecretDraft(record: SshKey): SshKeyDraft;
export function createSecretDraft(
    record: EditableSecret | null,
    kind?: EditableKind,
): SecretDraft;
export function createSecretDraft(
    record: EditableSecret | null,
    kind: EditableKind = "login",
): SecretDraft {
    const common = {
        name: record?.name ?? "",
        fields: record?.fields.map((field) => ({ ...field })) ?? [],
        notes: record?.notes ?? "",
        favorite: record?.favorite ?? false,
    };
    if ((record?.kind ?? kind) === "secureNote") {
        return { ...common, kind: "secureNote", type: 0 };
    }
    if ((record?.kind ?? kind) === "card") {
        const card = record?.kind === "card" ? record : null;
        return {
            ...common,
            kind: "card",
            cardholderName: card?.cardholderName ?? "",
            brand: card?.brand ?? "",
            number: card?.number ?? "",
            expMonth: card?.expMonth ?? "",
            expYear: card?.expYear ?? "",
            code: card?.code ?? "",
        };
    }
    if ((record?.kind ?? kind) === "identity") {
        const identity = record?.kind === "identity" ? record : null;
        return {
            ...common,
            kind: "identity",
            ...Object.fromEntries(
                identityFields.map(([key]) => [key, identity?.[key] ?? ""]),
            ),
        } as IdentityDraft;
    }
    if ((record?.kind ?? kind) === "sshKey") {
        const key = record?.kind === "sshKey" ? record : null;
        return {
            ...common,
            kind: "sshKey",
            privateKey: key?.privateKey ?? "",
            publicKey: key?.publicKey ?? "",
            keyFingerprint: key?.keyFingerprint ?? "",
        };
    }
    const login = record?.kind === "login" ? record : null;
    return {
        ...common,
        kind: "login",
        secret: login?.secret ?? "",
        username: login?.username ?? "",
        totp: login?.totp ?? "",
        uris: login?.uris.map((uri) => ({ ...uri })) ?? [],
    };
}

export function customFieldIssue(
    previous: EditableSecret | null,
    field: Secret["fields"][number],
    index: number,
): string | null {
    const old = previous?.fields[index];
    if (
        field.type !== "boolean" ||
        field.value === "true" ||
        field.value === "false" ||
        (old?.type === "boolean" &&
            old.name === field.name &&
            old.value === field.value)
    )
        return null;
    return `Custom field ${index + 1}: Select True or False, change the field type, or remove the field before saving.`;
}

export function constructSecret(
    previous: EditableSecret | null,
    draft: SecretDraft,
    id: string,
    now: string,
) {
    const fieldIssue = draft.fields
        .map((field, index) => customFieldIssue(previous, field, index))
        .find(Boolean);
    const fields = v.safeParse(
        v.pipe(
            v.array(CustomFieldSchema),
            v.check(() => !fieldIssue, fieldIssue ?? "Invalid custom field."),
        ),
        draft.fields,
    );
    if (!fields.success)
        return { success: false as const, issues: fields.issues };
    const common = {
        ...previous,
        ...draft,
        id,
        notes:
            previous && draft.notes === (previous.notes ?? "")
                ? previous.notes
                : draft.notes,
        history: previous?.history ?? [],
        createdAt: previous?.createdAt ?? now,
        updatedAt: now,
    };
    if (draft.kind === "secureNote")
        return v.safeParse(SecureNoteSchema, common);
    if (draft.kind === "card") {
        const card = previous?.kind === "card" ? previous : null;
        return v.safeParse(CardSchema, {
            ...common,
            ...Object.fromEntries(
                cardKeys.map((key) => [
                    key,
                    card && draft[key] === (card[key] ?? "")
                        ? card[key]
                        : draft[key],
                ]),
            ),
        });
    }
    if (draft.kind === "identity") {
        const identity = previous?.kind === "identity" ? previous : null;
        return v.safeParse(IdentitySchema, {
            ...common,
            ...Object.fromEntries(
                identityFields.map(([key]) => [
                    key,
                    identity && draft[key] === (identity[key] ?? "")
                        ? identity[key]
                        : draft[key],
                ]),
            ),
        });
    }
    if (draft.kind === "sshKey") {
        const key = previous?.kind === "sshKey" ? previous : null;
        return v.safeParse(SshKeySchema, {
            ...common,
            ...Object.fromEntries(
                (["publicKey", "keyFingerprint"] as const).map((field) => [
                    field,
                    key && draft[field] === (key[field] ?? "")
                        ? key[field]
                        : draft[field],
                ]),
            ),
        });
    }
    const login = previous?.kind === "login" ? previous : null;
    return v.safeParse(LoginSchema, {
        ...common,
        ...Object.fromEntries(
            (["secret", "username", "totp"] as const).map((key) => [
                key,
                login && draft[key] === (login[key] ?? "")
                    ? login[key]
                    : draft[key],
            ]),
        ),
        uris: draft.uris.filter(({ uri }) => uri.trim().length > 0),
        history:
            login && login.secret != null && login.secret !== draft.secret
                ? [...login.history, { secret: login.secret, changedAt: now }]
                : (previous?.history ?? []),
    });
}
