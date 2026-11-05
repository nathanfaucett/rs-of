import * as v from "valibot";
import { type Login, LoginSchema } from "./secret";

export type SecretDraft = Pick<
    Login,
    "name" | "uris" | "fields" | "favorite"
> & {
    secret: string;
    username: string;
    totp: string;
    notes: string;
};

export function createSecretDraft(record: Login | null): SecretDraft {
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
    previous: Login | null,
    draft: SecretDraft,
    id: string,
    now: string,
) {
    return v.safeParse(LoginSchema, {
        kind: "login",
        ...previous,
        ...draft,
        id,
        ...Object.fromEntries(
            (["secret", "username", "totp", "notes"] as const).map((key) => [
                key,
                previous && draft[key] === (previous[key] ?? "")
                    ? previous[key]
                    : draft[key],
            ]),
        ),
        uris: draft.uris.filter(({ uri }) => uri.trim().length > 0),
        history:
            previous &&
            previous.secret != null &&
            previous.secret !== draft.secret
                ? [
                      ...previous.history,
                      { secret: previous.secret, changedAt: now },
                  ]
                : (previous?.history ?? []),
        createdAt: previous?.createdAt ?? now,
        updatedAt: now,
    });
}
