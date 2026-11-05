import * as v from "valibot";
import {
    type Login,
    LoginSchema,
    type SecureNote,
    SecureNoteSchema,
} from "./secret";

export type EditableSecret = Login | SecureNote;
export type EditableKind = EditableSecret["kind"];
type CommonDraft = Pick<Login, "name" | "fields" | "favorite"> & { notes: string };
export type LoginDraft = CommonDraft & Pick<Login, "kind" | "uris"> & {
    secret: string;
    username: string;
    totp: string;
};
export type SecureNoteDraft = CommonDraft & Pick<SecureNote, "kind" | "type">;
export type SecretDraft = LoginDraft | SecureNoteDraft;

export function createSecretDraft(record: Login | null): LoginDraft;
export function createSecretDraft(record: SecureNote): SecureNoteDraft;
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

export function constructSecret(
    previous: EditableSecret | null,
    draft: SecretDraft,
    id: string,
    now: string,
) {
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
    if (draft.kind === "secureNote") return v.safeParse(SecureNoteSchema, common);
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
