import type {
    DatabaseResource,
    EnsureDatabaseRequest,
    SqlResponse,
} from "@aicacia/storage-client";
import * as v from "valibot";
import { type SqlStorage, storageRequest } from "./sql";

const schemaId = "f4077b65-9a42-5ade-b648-299c09235271";
const versionQuery = {
    sql: "SELECT id, version FROM secret_schema",
    params: [],
};

export async function prepareSecrets(
    client: SqlStorage & {
        ensureDatabase(
            request: EnsureDatabaseRequest,
            signal?: AbortSignal,
        ): Promise<DatabaseResource>;
    },
    signal: AbortSignal,
): Promise<DatabaseResource> {
    const database = await storageRequest(
        (current) =>
            client.ensureDatabase(
                {
                    creationKey: "password-manager:default",
                    name: "Password Manager",
                },
                current,
            ),
        signal,
    );
    signal.throwIfAborted();
    if (!v.is(v.object({ id: v.pipe(v.string(), v.uuid()) }), database))
        throw new Error("Malformed database resource");
    const execute = (
        statements: Parameters<SqlStorage["executeSql"]>[1]["statements"],
        actions: ("read" | "write")[],
    ) =>
        storageRequest(
            (current) =>
                client.executeSql(
                    database.id,
                    { statements },
                    actions,
                    current,
                ),
            signal,
        );
    const catalog = await execute(
        [{ sql: "SELECT name FROM tables", params: [] }],
        ["read"],
    );
    const result = catalog.results[0];
    if (
        catalog.results.length !== 1 ||
        result?.columns.join() !== "name" ||
        !Array.isArray(result.rows)
    )
        throw new Error("Malformed database schema");
    const names = result.rows.map((row) =>
        row.length === 1 && row[0]?.type === "text" ? row[0].value : null,
    );
    if (!names.length) {
        const initialized = await execute(
            [
                {
                    sql: "CREATE TABLE IF NOT EXISTS secret_schema (id UUID PRIMARY KEY, version INTEGER NOT NULL)",
                    params: [],
                },
                {
                    sql: "CREATE TABLE IF NOT EXISTS secrets (id UUID PRIMARY KEY, content TEXT NOT NULL)",
                    params: [],
                },
                {
                    sql: "INSERT INTO secret_schema (id, version) VALUES (?, ?) ON CONFLICT (id) DO NOTHING",
                    params: [
                        { type: "uuid", value: schemaId },
                        { type: "integer", value: "1" },
                    ],
                },
                versionQuery,
            ],
            ["read", "write"],
        );
        checkVersion(initialized, 3);
    } else {
        if (
            names.length !== 2 ||
            !names.includes("secret_schema") ||
            !names.includes("secrets")
        )
            throw new Error(
                "Malformed database schema; automatic repair is disabled",
            );
        checkVersion(await execute([versionQuery], ["read"]), 0);
    }
    signal.throwIfAborted();
    return database;
}

function checkVersion(response: SqlResponse, index: number) {
    const result = response.results[index];
    const row = result?.rows[0];
    if (
        response.results.length !== index + 1 ||
        result?.columns.join() !== "id,version" ||
        result.rows.length !== 1 ||
        row?.length !== 2 ||
        row[0]?.type !== "uuid" ||
        row[0].value !== schemaId ||
        row[1]?.type !== "integer" ||
        row[1].value !== "1"
    )
        throw new Error(
            "Incompatible or malformed database schema. No migration or reset was attempted.",
        );
}
