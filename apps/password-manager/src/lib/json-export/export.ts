import * as v from "valibot";
import { SecretSchema, type Secret } from "../secrets/secret";
import {
    FolderSchema,
    validateItemSnapshot,
    type ItemSnapshot,
} from "../vault/folder";

export class JsonExportError extends Error {}

const snapshotSchema = v.strictObject({
    items: v.array(SecretSchema),
    folders: v.array(FolderSchema),
});
const uriTypes = {
    domain: 0,
    host: 1,
    startsWith: 2,
    exact: 3,
    regex: 4,
    never: 5,
};
const fieldTypes = { text: 0, hidden: 1, boolean: 2 };
const itemTypes = { login: 1, secureNote: 2, card: 3, identity: 4, sshKey: 5 };
const cardKeys = [
    "cardholderName",
    "brand",
    "number",
    "expMonth",
    "expYear",
    "code",
] as const;
const identityKeys = [
    "title",
    "firstName",
    "middleName",
    "lastName",
    "address1",
    "address2",
    "address3",
    "city",
    "state",
    "postalCode",
    "country",
    "company",
    "email",
    "phone",
    "ssn",
    "username",
    "passportNumber",
    "licenseNumber",
] as const;
const sshKeys = ["privateKey", "publicKey", "keyFingerprint"] as const;

function date(value: string, path: string) {
    const [year, month, day] = value.slice(0, 10).split("-").map(Number);
    const leap = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
    const days = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][
        month - 1
    ];
    if (
        !days ||
        day < 1 ||
        day > days ||
        !Number.isFinite(Date.parse(value)) ||
        /\.\d{3}\d*[1-9]\d*(?:Z|[+-]\d\d:\d\d)$/.test(value)
    )
        throw new JsonExportError(
            `Cannot represent date at ${path}. Export blocked; use an encrypted backup.`,
        );
    return new Date(value).toISOString();
}

export function inspectJsonExport(input: ItemSnapshot) {
    const result = v.safeParse(snapshotSchema, input);
    if (!result.success) {
        const paths = [
            ...new Set(
                result.issues.map(
                    (issue) =>
                        "/" +
                        (issue.path
                            ?.map((part) =>
                                String(part.key)
                                    .replaceAll("~", "~0")
                                    .replaceAll("/", "~1"),
                            )
                            .join("/") ?? ""),
                ),
            ),
        ];
        throw new JsonExportError(
            `Unsupported or invalid data at ${paths.join(", ")}. Export blocked; use an encrypted backup.`,
        );
    }
    const snapshot = validateItemSnapshot(result.output);
    const counts = { login: 0, secureNote: 0, card: 0, identity: 0, sshKey: 0 };
    let trash = 0;
    snapshot.items.forEach((item, index) => {
        if (item.deletedAt) {
            trash++;
            return;
        }
        counts[item.kind]++;
        date(item.createdAt, `/items/${index}/createdAt`);
        date(item.updatedAt, `/items/${index}/updatedAt`);
        item.history.forEach((entry, historyIndex) => {
            date(
                entry.changedAt,
                `/items/${index}/history/${historyIndex}/changedAt`,
            );
        });
        item.fields.forEach((field, fieldIndex) => {
            if (
                field.type === "boolean" &&
                field.value !== "true" &&
                field.value !== "false"
            )
                throw new JsonExportError(
                    `Cannot represent Boolean at /items/${index}/fields/${fieldIndex}/value. Export blocked; use an encrypted backup.`,
                );
        });
    });
    return {
        snapshot,
        counts,
        trash,
        active: snapshot.items.length - trash,
        folders: snapshot.folders.length,
    };
}

function payload(item: Secret) {
    switch (item.kind) {
        case "login":
            return {
                login: {
                    username: item.username ?? null,
                    password: item.secret ?? null,
                    totp: item.totp ?? null,
                    uris: item.uris.map((uri) => ({
                        uri: uri.uri,
                        match: uri.match == null ? null : uriTypes[uri.match],
                    })),
                    fido2Credentials: [],
                },
            };
        case "secureNote":
            return { secureNote: { type: 0 } };
        case "card":
            return {
                card: Object.fromEntries(
                    cardKeys.map((key) => [key, item[key] ?? null]),
                ),
            };
        case "identity":
            return {
                identity: Object.fromEntries(
                    identityKeys.map((key) => [key, item[key] ?? null]),
                ),
            };
        case "sshKey":
            return {
                sshKey: Object.fromEntries(
                    sshKeys.map((key) => [key, item[key] ?? null]),
                ),
            };
    }
}

export function serializeJsonExport(input: ItemSnapshot): string {
    const { snapshot } = inspectJsonExport(input);
    return JSON.stringify(
        {
            encrypted: false,
            folders: snapshot.folders,
            items: snapshot.items
                .filter((item) => !item.deletedAt)
                .map((item) => ({
                    id: item.id,
                    organizationId: null,
                    collectionIds: [],
                    folderId: item.folderId ?? null,
                    type: itemTypes[item.kind],
                    reprompt: 0,
                    name: item.name,
                    notes: item.notes ?? null,
                    favorite: item.favorite,
                    fields: item.fields.map((field) => ({
                        name: field.name,
                        value: field.value,
                        type: fieldTypes[field.type],
                    })),
                    passwordHistory: item.history.map((entry) => ({
                        password: entry.secret,
                        lastUsedDate: date(
                            entry.changedAt,
                            "/history/changedAt",
                        ),
                    })),
                    creationDate: date(item.createdAt, "/createdAt"),
                    revisionDate: date(item.updatedAt, "/updatedAt"),
                    deletedDate: null,
                    archivedDate: null,
                    ...payload(item),
                })),
        },
        null,
        2,
    );
}
