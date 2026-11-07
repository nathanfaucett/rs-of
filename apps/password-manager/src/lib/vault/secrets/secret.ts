import * as v from "valibot";

export const UriMatchType = {
    Domain: "domain",
    Host: "host",
    StartsWith: "startsWith",
    Exact: "exact",
    RegularExpression: "regex",
    Never: "never",
} as const;
export type UriMatchType = (typeof UriMatchType)[keyof typeof UriMatchType];
export const UriMatchTypeSchema = v.picklist(Object.values(UriMatchType));
export const FieldType = {
    Text: "text",
    Hidden: "hidden",
    Boolean: "boolean",
} as const;
export type FieldType = (typeof FieldType)[keyof typeof FieldType];
export const FieldTypeSchema = v.picklist(Object.values(FieldType));
const optionalText = v.optional(v.nullable(v.string()));
const nonblank = v.pipe(
    v.string(),
    v.check((value) => value.trim().length > 0),
);
const date = v.pipe(v.string(), v.isoTimestamp());
export const SecretIdSchema = v.pipe(v.string(), v.uuid());
export const UriSchema = v.strictObject({
    uri: nonblank,
    match: v.optional(v.nullable(UriMatchTypeSchema), null),
});
export const CustomFieldSchema = v.strictObject({
    name: nonblank,
    value: v.nullable(v.string()),
    type: FieldTypeSchema,
});
export const SecretHistoryEntrySchema = v.strictObject({
    secret: v.string(),
    changedAt: date,
});
const common = {
    id: SecretIdSchema,
    name: nonblank,
    notes: optionalText,
    favorite: v.optional(v.boolean(), false),
    fields: v.optional(v.array(CustomFieldSchema), []),
    history: v.optional(v.array(SecretHistoryEntrySchema), []),
    createdAt: date,
    updatedAt: date,
    deletedAt: v.optional(date),
    folderId: v.optional(v.nullable(SecretIdSchema)),
    importSource: v.optional(
        v.strictObject({
            format: v.literal("personal-json-v1"),
            id: nonblank,
        }),
    ),
};
export const LoginSchema = v.strictObject({
    ...common,
    kind: v.literal("login"),
    secret: optionalText,
    username: optionalText,
    totp: optionalText,
    uris: v.optional(v.array(UriSchema), []),
});
export const CardSchema = v.strictObject({
    ...common,
    kind: v.literal("card"),
    cardholderName: optionalText,
    brand: optionalText,
    number: optionalText,
    expMonth: optionalText,
    expYear: optionalText,
    code: optionalText,
});
export const IdentitySchema = v.strictObject({
    ...common,
    kind: v.literal("identity"),
    title: optionalText,
    firstName: optionalText,
    middleName: optionalText,
    lastName: optionalText,
    address1: optionalText,
    address2: optionalText,
    address3: optionalText,
    city: optionalText,
    state: optionalText,
    postalCode: optionalText,
    country: optionalText,
    company: optionalText,
    email: optionalText,
    phone: optionalText,
    ssn: optionalText,
    username: optionalText,
    passportNumber: optionalText,
    licenseNumber: optionalText,
});
export const SecureNoteSchema = v.strictObject({
    ...common,
    kind: v.literal("secureNote"),
    type: v.optional(v.literal(0), 0),
});
export const SshKeySchema = v.strictObject({
    ...common,
    kind: v.literal("sshKey"),
    privateKey: nonblank,
    publicKey: optionalText,
    keyFingerprint: optionalText,
});
export const SecretSchema = v.variant("kind", [
    LoginSchema,
    CardSchema,
    IdentitySchema,
    SecureNoteSchema,
    SshKeySchema,
]);
export type Secret = v.InferOutput<typeof SecretSchema>;
export type Login = v.InferOutput<typeof LoginSchema>;

export type Card = v.InferOutput<typeof CardSchema>;
export type Identity = v.InferOutput<typeof IdentitySchema>;
export type SecureNote = v.InferOutput<typeof SecureNoteSchema>;
export type SshKey = v.InferOutput<typeof SshKeySchema>;
