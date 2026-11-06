import * as v from "valibot";
import { normalizeTimestamp } from "$lib/timestamp";
import {
    SecretSchema,
    type Secret,
    type UriMatchType,
} from "$lib/secrets/secret";
import type { Folder } from "$lib/vault/folder";

export const IMPORT_MAX_BYTES = 16 * 1024 * 1024;
export const IMPORT_MAX_RECORDS = 10_000;
export type ImportIssue = {
    path: string;
    message: string;
    unsupported: boolean;
};
export type ParsedImport = {
    items: Secret[];
    folders: Folder[];
    issues: ImportIssue[];
    warnings: string[];
    missingDates: Set<string>;
};
type ObjectValue = Record<string, unknown>;
const pointer = (path: string, key: string | number) =>
    `${path}/${String(key).replaceAll("~", "~0").replaceAll("/", "~1")}`;

// JSON.parse cannot detect duplicate members, including escaped spellings.
function strictJson(text: string): unknown {
    let offset = 0;
    const space = () => {
        while (/[\t\n\r ]/.test(text[offset] ?? "x")) offset++;
    };
    const fail = (_path: string): never => {
        throw {
            path: "/",
            message: "Invalid JSON or nesting exceeds 64 levels.",
            unsupported: false,
        };
    };
    const string = (path: string): string => {
        const start = offset++;
        while (offset < text.length) {
            const char = text[offset++];
            if (char === "\\") offset++;
            else if (char === '"') {
                try {
                    return JSON.parse(text.slice(start, offset));
                } catch {
                    fail(path);
                }
            }
        }
        return fail(path);
    };
    const value = (path: string, depth: number): unknown => {
        if (depth > 64) fail(path);
        space();
        const char = text[offset];
        if (char === '"') return string(path);
        if (char === "{" || char === "[") {
            offset++;
            const object: ObjectValue = Object.create(null);
            const array: unknown[] = [];
            const end = char === "{" ? "}" : "]";
            space();
            if (text[offset] === end) {
                offset++;
                return char === "{" ? object : array;
            }
            while (offset < text.length) {
                space();
                if (char === "{") {
                    if (text[offset] !== '"') fail(path);
                    const key = string(path);
                    const child = pointer(path, key);
                    if (Object.hasOwn(object, key))
                        throw {
                            path: child,
                            message: "Duplicate JSON member.",
                            unsupported: false,
                        };
                    space();
                    if (text[offset++] !== ":") fail(path);
                    object[key] = value(child, depth + 1);
                } else
                    array.push(value(pointer(path, array.length), depth + 1));
                space();
                if (text[offset] === end) {
                    offset++;
                    return char === "{" ? object : array;
                }
                if (text[offset++] !== ",") fail(path);
            }
            fail(path);
        }
        const token =
            /^(?:true|false|null|-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?)/.exec(
                text.slice(offset),
            );
        if (!token) return fail(path);
        offset += token[0].length;
        return JSON.parse(token[0]);
    };
    const result = value("", 0);
    space();
    if (offset !== text.length) fail("");
    return result;
}

const kinds = ["login", "secureNote", "card", "identity", "sshKey"] as const;
const cardKeys = [
    "cardholderName",
    "brand",
    "number",
    "expMonth",
    "expYear",
    "code",
];
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
];
const rules: UriMatchType[] = [
    "domain",
    "host",
    "startsWith",
    "exact",
    "regex",
    "never",
];

export function parsePersonalJson(
    text: string,
    now = new Date().toISOString(),
): ParsedImport {
    const result: ParsedImport = {
        items: [],
        folders: [],
        issues: [],
        warnings: [],
        missingDates: new Set(),
    };
    const issue = (
        path: string,
        message = "Invalid field shape or value.",
        unsupported = false,
    ) => {
        result.issues.push({ path: path || "/", message, unsupported });
    };
    if (new TextEncoder().encode(text).length > IMPORT_MAX_BYTES) {
        issue(
            "/",
            "File exceeds 16 MiB. Split it into smaller supported JSON files.",
        );
        return result;
    }
    let root: unknown;
    try {
        root = strictJson(text);
    } catch (error) {
        result.issues.push(error as ImportIssue);
        return result;
    }
    const object = (
        value: unknown,
        path: string,
        keys: readonly string[],
    ): ObjectValue => {
        if (!value || typeof value !== "object" || Array.isArray(value)) {
            issue(path, "Expected an object.");
            return Object.create(null);
        }
        for (const key of Object.keys(value))
            if (!keys.includes(key))
                issue(pointer(path, key), "Unsupported field.", true);
        return value as ObjectValue;
    };
    const array = (
        value: unknown,
        path: string,
        optional = true,
    ): unknown[] => {
        if (optional && value == null) return [];
        if (!Array.isArray(value)) {
            issue(path, "Expected an array.");
            return [];
        }
        return value;
    };
    const nonblank = (value: unknown, path: string): string => {
        if (typeof value !== "string" || !value.trim()) {
            issue(path, "Expected nonblank text.");
            return "";
        }
        return value;
    };
    const textValue = (
        value: unknown,
        path: string,
    ): string | null | undefined => {
        if (typeof value === "string") return value;
        if (value === null) return null;
        if (value === undefined) return undefined;
        issue(path, "Expected text or null.");
        return undefined;
    };
    const empty = (
        value: unknown,
        path: string,
        allowArray = false,
        allowZero = false,
    ) => {
        if (
            value == null ||
            (allowArray && Array.isArray(value) && !value.length) ||
            (allowZero && value === 0)
        )
            return;
        issue(path, "Unsupported data or policy.", true);
    };
    const date = (value: unknown, path: string, fallback?: string): string => {
        if (value == null && fallback) {
            result.warnings.push(
                `${path}: date unavailable; import time will be used.`,
            );
            return fallback;
        }
        if (typeof value === "string") {
            try {
                return normalizeTimestamp(value);
            } catch {
                // Report the path below, never the source value.
            }
        }
        issue(path, "Expected a valid RFC 3339 date with timezone.");
        return now;
    };
    const envelope = object(root, "", [
        "encrypted",
        "folders",
        "items",
        "collections",
    ]);
    if (envelope.encrypted !== undefined && envelope.encrypted !== false)
        issue("/encrypted", "Only plaintext personal JSON is supported.", true);
    if (
        envelope.collections !== undefined &&
        (!Array.isArray(envelope.collections) || envelope.collections.length)
    )
        issue("/collections", "Collections are unsupported.", true);
    const folders =
        envelope.folders === undefined
            ? []
            : array(envelope.folders, "/folders", false);
    const items = array(envelope.items, "/items", false);
    if (
        folders.length > IMPORT_MAX_RECORDS ||
        items.length > IMPORT_MAX_RECORDS
    ) {
        issue("/", "Limit: 10,000 items and 10,000 folders. Split the input.");
        return result;
    }
    const folderMap = new Map<string, string>();
    folders.forEach((value, index) => {
        const path = `/folders/${index}`;
        const folder = object(value, path, ["id", "name"]);
        const source = nonblank(folder.id, `${path}/id`);
        if (folderMap.has(source))
            issue(`${path}/id`, "Duplicate folder identity.");
        const id = crypto.randomUUID();
        folderMap.set(source, id);
        result.folders.push({
            id,
            name: nonblank(folder.name, `${path}/name`),
        });
    });
    const sourceIds = new Set<string>();
    items.forEach((value, index) => {
        const path = `/items/${index}`;
        const item = object(value, path, [
            "id",
            "type",
            "name",
            "folderId",
            "notes",
            "favorite",
            "fields",
            "passwordHistory",
            "creationDate",
            "revisionDate",
            "deletedDate",
            "archivedDate",
            "reprompt",
            "organizationId",
            "collectionIds",
            ...kinds,
        ]);
        const kind =
            typeof item.type === "number" && Number.isInteger(item.type)
                ? kinds[item.type - 1]
                : undefined;
        if (!kind) issue(`${path}/type`, "Unsupported item type.", true);
        for (const other of kinds)
            if (other !== kind && Object.hasOwn(item, other))
                issue(`${path}/${other}`, "Mismatched type payload.", true);
        empty(item.organizationId, `${path}/organizationId`);
        empty(item.collectionIds, `${path}/collectionIds`, true);
        empty(item.deletedDate, `${path}/deletedDate`);
        empty(item.archivedDate, `${path}/archivedDate`);
        empty(item.reprompt, `${path}/reprompt`, false, true);
        const id = crypto.randomUUID();
        let importSource;
        if (item.id !== undefined) {
            const source = nonblank(item.id, `${path}/id`);
            if (sourceIds.has(source))
                issue(`${path}/id`, "Duplicate source identity.");
            sourceIds.add(source);
            importSource = { format: "personal-json-v1", id: source };
        }
        let folderId: string | null = null;
        if (item.folderId != null) {
            if (
                typeof item.folderId !== "string" ||
                !folderMap.has(item.folderId)
            )
                issue(
                    `${path}/folderId`,
                    "Missing or ambiguous folder reference.",
                );
            else folderId = folderMap.get(item.folderId)!;
        }
        if (item.favorite !== undefined && typeof item.favorite !== "boolean")
            issue(`${path}/favorite`, "Expected a boolean.");
        const fields = array(item.fields, `${path}/fields`).map(
            (value, index) => {
                const fieldPath = `${path}/fields/${index}`;
                const field = object(value, fieldPath, [
                    "name",
                    "value",
                    "type",
                    "linkedId",
                ]);
                empty(field.linkedId, `${fieldPath}/linkedId`);
                const type =
                    field.type === 0
                        ? "text"
                        : field.type === 1
                          ? "hidden"
                          : field.type === 2
                            ? "boolean"
                            : undefined;
                if (!type)
                    issue(
                        field.type === 3 ? fieldPath : `${fieldPath}/type`,
                        "Unsupported custom field type.",
                        true,
                    );
                if (
                    type === "boolean" &&
                    field.value !== "true" &&
                    field.value !== "false"
                )
                    issue(
                        `${fieldPath}/value`,
                        'Boolean fields require the string "true" or "false".',
                    );
                return {
                    name: nonblank(field.name, `${fieldPath}/name`),
                    type,
                    value: textValue(field.value, `${fieldPath}/value`) ?? null,
                };
            },
        );
        const history = array(
            item.passwordHistory,
            `${path}/passwordHistory`,
        ).map((value, index) => {
            const historyPath = `${path}/passwordHistory/${index}`;
            const entry = object(value, historyPath, [
                "password",
                "lastUsedDate",
            ]);
            if (typeof entry.password !== "string")
                issue(`${historyPath}/password`, "Expected text.");
            return {
                secret: entry.password,
                changedAt: date(
                    entry.lastUsedDate,
                    `${historyPath}/lastUsedDate`,
                ),
            };
        });
        if (item.creationDate == null)
            result.missingDates.add(`${id}/createdAt`);
        if (item.revisionDate == null)
            result.missingDates.add(`${id}/updatedAt`);
        const common = {
            id,
            kind,
            name: nonblank(item.name, `${path}/name`),
            notes: textValue(item.notes, `${path}/notes`),
            favorite: item.favorite ?? false,
            fields,
            history,
            folderId,
            ...(importSource ? { importSource } : {}),
            createdAt: date(item.creationDate, `${path}/creationDate`, now),
            updatedAt: date(item.revisionDate, `${path}/revisionDate`, now),
        };
        const payloadPath = `${path}/${kind}`;
        let details: ObjectValue = {};
        if (kind === "login") {
            const login = object(item.login, payloadPath, [
                "username",
                "password",
                "totp",
                "uris",
                "fido2Credentials",
            ]);
            empty(
                login.fido2Credentials,
                `${payloadPath}/fido2Credentials`,
                true,
            );
            details = {
                username: textValue(login.username, `${payloadPath}/username`),
                secret: textValue(login.password, `${payloadPath}/password`),
                totp: textValue(login.totp, `${payloadPath}/totp`),
                uris: array(login.uris, `${payloadPath}/uris`).map(
                    (value, index) => {
                        const uriPath = `${payloadPath}/uris/${index}`;
                        const uri = object(value, uriPath, ["uri", "match"]);
                        let match: UriMatchType | null = null;
                        if (uri.match != null) {
                            if (
                                typeof uri.match !== "number" ||
                                !Number.isInteger(uri.match) ||
                                !rules[uri.match]
                            )
                                issue(
                                    `${uriPath}/match`,
                                    "Unsupported URI strategy.",
                                    true,
                                );
                            else match = rules[uri.match];
                        }
                        return {
                            uri: nonblank(uri.uri, `${uriPath}/uri`),
                            match,
                        };
                    },
                ),
            };
        } else if (kind === "secureNote") {
            const note = object(item.secureNote, payloadPath, ["type"]);
            if (note.type !== undefined && note.type !== 0)
                issue(`${payloadPath}/type`, "Unsupported note subtype.", true);
            details = { type: 0 };
        } else if (kind) {
            const keys =
                kind === "card"
                    ? cardKeys
                    : kind === "identity"
                      ? identityKeys
                      : ["privateKey", "publicKey", "keyFingerprint"];
            const payload = object(item[kind], payloadPath, keys);
            for (const key of keys)
                details[key] =
                    kind === "sshKey" && key === "privateKey"
                        ? nonblank(payload[key], `${payloadPath}/${key}`)
                        : textValue(payload[key], `${payloadPath}/${key}`);
        }
        const parsed = v.safeParse(SecretSchema, { ...common, ...details });
        if (parsed.success) result.items.push(parsed.output);
        else if (
            !result.issues.some((entry) => entry.path.startsWith(`${path}/`))
        )
            issue(path, "Item cannot be represented without loss.");
    });
    if (
        result.items.some(
            (item) =>
                item.kind === "login" &&
                item.uris.some((uri) => uri.match === null),
        )
    )
        result.warnings.push(
            "Inherited URI rules use this vault's base-domain default. Source account defaults and equivalent domains are not in the file. URI rules are stored, not executed.",
        );
    return result;
}
