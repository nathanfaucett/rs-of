import * as v from "valibot";
import { type Secret, SecretSchema } from "./secret";

export type SecretDraft = Pick<
    Secret,
    "name" | "secret" | "uris" | "fields" | "favorite"
> & {
    username: string;
    totp: string;
    notes: string;
};

export function createSecretDraft(record: Secret | null): SecretDraft {
    return {
        name: record?.name ?? "",
        secret: record?.secret ?? "",
        username: record?.username ?? "",
        totp: record?.totp ?? "",
        uris: record?.uris.map((uri) => ({ ...uri })) ?? [],
        fields: record?.fields.map((field) => ({ ...field })) ?? [],
        notes: record?.notes ?? "",
        favorite: record?.favorite ?? false,
    };
}

export function constructSecret(
    previous: Secret | null,
    draft: SecretDraft,
    id: string,
    now: string,
) {
    return v.safeParse(SecretSchema, {
        ...previous,
        ...draft,
        id,
        uris: draft.uris.filter(({ uri }) => uri.trim().length > 0),
        history:
            previous && previous.secret !== draft.secret
                ? [
                      ...previous.history,
                      { secret: previous.secret, changedAt: now },
                  ]
                : (previous?.history ?? []),
        createdAt: previous?.createdAt ?? now,
        updatedAt: now,
    });
}
