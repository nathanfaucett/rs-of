import * as v from "valibot";
import { normalizeTimestamp } from "../../shared/util/timestamp";
import type { Folder } from "../folder";
import { type Secret, SecretSchema, type UriMatchType } from "../secrets/secret";

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
const property = (value: ObjectValue, key: string): unknown => value[key];
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
        } else array.push(value(pointer(path, array.length), depth + 1));
        space();
        if (text[offset] === end) {
          offset++;
          return char === "{" ? object : array;
        }
        if (text[offset++] !== ",") fail(path);
      }
      fail(path);
    }
    const token = /^(?:true|false|null|-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?)/.exec(
      text.slice(offset)
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
const cardKeys = ["cardholderName", "brand", "number", "expMonth", "expYear", "code"];
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
const rules: UriMatchType[] = ["domain", "host", "startsWith", "exact", "regex", "never"];

export function parsePersonalJson(text: string, now = new Date().toISOString()): ParsedImport {
  const result: ParsedImport = {
    items: [],
    folders: [],
    issues: [],
    warnings: [],
    missingDates: new Set(),
  };
  const issue = (path: string, message = "Invalid field shape or value.", unsupported = false) => {
    result.issues.push({ path: path || "/", message, unsupported });
  };
  if (new TextEncoder().encode(text).length > IMPORT_MAX_BYTES) {
    issue("/", "File exceeds 16 MiB. Split it into smaller supported JSON files.");
    return result;
  }
  let root: unknown;
  try {
    root = strictJson(text);
  } catch (error) {
    result.issues.push(error as ImportIssue);
    return result;
  }
  const object = (value: unknown, path: string, keys: readonly string[]): ObjectValue => {
    if (!value || typeof value !== "object" || Array.isArray(value)) {
      issue(path, "Expected an object.");
      return Object.create(null);
    }
    for (const key of Object.keys(value))
      if (!keys.includes(key)) issue(pointer(path, key), "Unsupported field.", true);
    return value as ObjectValue;
  };
  const array = (value: unknown, path: string, optional = true): unknown[] => {
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
  const textValue = (value: unknown, path: string): string | null | undefined => {
    if (typeof value === "string") return value;
    if (value === null) return null;
    if (value === undefined) return undefined;
    issue(path, "Expected text or null.");
    return undefined;
  };
  const empty = (value: unknown, path: string, allowArray = false, allowZero = false) => {
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
      result.warnings.push(`${path}: date unavailable; import time will be used.`);
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
  const envelope = object(root, "", ["encrypted", "folders", "items", "collections"]);
  const encrypted = property(envelope, "encrypted");
  if (encrypted !== undefined && encrypted !== false)
    issue("/encrypted", "Only plaintext personal JSON is supported.", true);
  const collections = property(envelope, "collections");
  if (collections !== undefined && (!Array.isArray(collections) || collections.length))
    issue("/collections", "Collections are unsupported.", true);
  const folderValues = property(envelope, "folders");
  const folders = folderValues === undefined ? [] : array(folderValues, "/folders", false);
  const items = array(property(envelope, "items"), "/items", false);
  if (folders.length > IMPORT_MAX_RECORDS || items.length > IMPORT_MAX_RECORDS) {
    issue("/", "Limit: 10,000 items and 10,000 folders. Split the input.");
    return result;
  }
  const folderMap = new Map<string, string>();
  folders.forEach((value, index) => {
    const path = `/folders/${index}`;
    const folder = object(value, path, ["id", "name"]);
    const source = nonblank(property(folder, "id"), `${path}/id`);
    if (folderMap.has(source)) issue(`${path}/id`, "Duplicate folder identity.");
    const id = crypto.randomUUID();
    folderMap.set(source, id);
    result.folders.push({
      id,
      name: nonblank(property(folder, "name"), `${path}/name`),
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
    const sourceKind = property(item, "type");
    const kind =
      typeof sourceKind === "number" && Number.isInteger(sourceKind)
        ? kinds[sourceKind - 1]
        : undefined;
    if (!kind) issue(`${path}/type`, "Unsupported item type.", true);
    for (const other of kinds)
      if (other !== kind && Object.hasOwn(item, other))
        issue(`${path}/${other}`, "Mismatched type payload.", true);
    empty(property(item, "organizationId"), `${path}/organizationId`);
    empty(property(item, "collectionIds"), `${path}/collectionIds`, true);
    empty(property(item, "deletedDate"), `${path}/deletedDate`);
    empty(property(item, "archivedDate"), `${path}/archivedDate`);
    empty(property(item, "reprompt"), `${path}/reprompt`, false, true);
    const id = crypto.randomUUID();
    let importSource: Secret["importSource"];
    const sourceId = property(item, "id");
    if (sourceId !== undefined) {
      const source = nonblank(sourceId, `${path}/id`);
      if (sourceIds.has(source)) issue(`${path}/id`, "Duplicate source identity.");
      sourceIds.add(source);
      importSource = { format: "personal-json-v1", id: source };
    }
    let folderId: string | null = null;
    const sourceFolderId = property(item, "folderId");
    if (sourceFolderId != null) {
      if (typeof sourceFolderId !== "string" || !folderMap.has(sourceFolderId))
        issue(`${path}/folderId`, "Missing or ambiguous folder reference.");
      else folderId = folderMap.get(sourceFolderId) ?? null;
    }
    const favorite = property(item, "favorite");
    if (favorite !== undefined && typeof favorite !== "boolean")
      issue(`${path}/favorite`, "Expected a boolean.");
    const fields = array(property(item, "fields"), `${path}/fields`).map((value, index) => {
      const fieldPath = `${path}/fields/${index}`;
      const field = object(value, fieldPath, ["name", "value", "type", "linkedId"]);
      empty(property(field, "linkedId"), `${fieldPath}/linkedId`);
      const sourceType = property(field, "type");
      const type =
        sourceType === 0
          ? "text"
          : sourceType === 1
            ? "hidden"
            : sourceType === 2
              ? "boolean"
              : undefined;
      if (!type)
        issue(
          sourceType === 3 ? fieldPath : `${fieldPath}/type`,
          "Unsupported custom field type.",
          true
        );
      if (
        type === "boolean" &&
        property(field, "value") !== "true" &&
        property(field, "value") !== "false"
      )
        issue(`${fieldPath}/value`, 'Boolean fields require the string "true" or "false".');
      return {
        name: nonblank(property(field, "name"), `${fieldPath}/name`),
        type,
        value: textValue(property(field, "value"), `${fieldPath}/value`) ?? null,
      };
    });
    const history = array(property(item, "passwordHistory"), `${path}/passwordHistory`).map(
      (value, index) => {
        const historyPath = `${path}/passwordHistory/${index}`;
        const entry = object(value, historyPath, ["password", "lastUsedDate"]);
        const password = property(entry, "password");
        if (typeof password !== "string") issue(`${historyPath}/password`, "Expected text.");
        return {
          secret: password,
          changedAt: date(property(entry, "lastUsedDate"), `${historyPath}/lastUsedDate`),
        };
      }
    );
    const creationDate = property(item, "creationDate");
    const revisionDate = property(item, "revisionDate");
    if (creationDate == null) result.missingDates.add(`${id}/createdAt`);
    if (revisionDate == null) result.missingDates.add(`${id}/updatedAt`);
    const common = {
      id,
      kind,
      name: nonblank(property(item, "name"), `${path}/name`),
      notes: textValue(property(item, "notes"), `${path}/notes`),
      favorite: favorite ?? false,
      fields,
      history,
      folderId,
      ...(importSource ? { importSource } : {}),
      createdAt: date(creationDate, `${path}/creationDate`, now),
      updatedAt: date(revisionDate, `${path}/revisionDate`, now),
    };
    const payloadPath = `${path}/${kind}`;
    let details: ObjectValue = {};
    if (kind === "login") {
      const login = object(property(item, "login"), payloadPath, [
        "username",
        "password",
        "totp",
        "uris",
        "fido2Credentials",
      ]);
      const passkeys = property(login, "fido2Credentials");
      if (passkeys != null && !(Array.isArray(passkeys) && passkeys.length === 0))
        result.warnings.push(
          `${payloadPath}/fido2Credentials: passkeys are not supported; skipped, never stored or shown.`
        );
      details = {
        username: textValue(property(login, "username"), `${payloadPath}/username`),
        secret: textValue(property(login, "password"), `${payloadPath}/password`),
        totp: textValue(property(login, "totp"), `${payloadPath}/totp`),
        uris: array(property(login, "uris"), `${payloadPath}/uris`).map((value, index) => {
          const uriPath = `${payloadPath}/uris/${index}`;
          const uri = object(value, uriPath, ["uri", "match"]);
          let match: UriMatchType | null = null;
          const sourceMatch = property(uri, "match");
          if (sourceMatch != null) {
            const rule =
              typeof sourceMatch === "number" && Number.isInteger(sourceMatch)
                ? rules[sourceMatch]
                : undefined;
            if (!rule) issue(`${uriPath}/match`, "Unsupported URI strategy.", true);
            else match = rule;
          }
          return {
            uri: nonblank(property(uri, "uri"), `${uriPath}/uri`),
            match,
          };
        }),
      };
    } else if (kind === "secureNote") {
      const note = object(property(item, "secureNote"), payloadPath, ["type"]);
      const noteType = property(note, "type");
      if (noteType !== undefined && noteType !== 0)
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
    else if (!result.issues.some((entry) => entry.path.startsWith(`${path}/`)))
      issue(path, "Item cannot be represented without loss.");
  });
  if (
    result.items.some(
      (item) => item.kind === "login" && item.uris.some((uri) => uri.match === null)
    )
  )
    result.warnings.push(
      "Inherited URI rules use this vault's base-domain default. Source account defaults and equivalent domains are not in the file. URI rules are stored, not executed."
    );
  return result;
}
