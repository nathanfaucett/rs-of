import * as v from "valibot";
import { LoginSchema, UriMatchTypeSchema } from "./secret";

// Read-only source format: omitted URI rules meant explicit base domain.
const legacyLogin = v.strictObject({
    ...v.omit(LoginSchema, ["kind", "folderId", "importSource"]).entries,
    secret: v.pipe(v.string(), v.minLength(1)),
    uris: v.optional(
        v.array(
            v.strictObject({
                uri: v.pipe(v.string(), v.minLength(1)),
                match: v.optional(UriMatchTypeSchema, "domain"),
            }),
        ),
        [],
    ),
});

export function transitionLogin(input: unknown) {
    return v.parse(LoginSchema, {
        ...v.parse(legacyLogin, input),
        kind: "login",
    });
}
