import type { SqlRequest, SqlValue } from "@aicacia/storage-client";
export const id = "01900000-0000-7000-8000-000000000001";
export const secret = {
    id,
    name: "First secret",
    secret: "private",
    favorite: false,
    notes: "",
    uris: [
        { uri: "https://first.test", match: "exact" },
        { uri: "https://second.test", match: "host" },
    ],
    fields: [{ name: "Account", value: "kept", type: "hidden" }],
    history: [{ secret: "older", changedAt: "2025-01-01T00:00:00.000Z" }],
    deletedAt: "2025-02-01T00:00:00.000Z",
    createdAt: "2026-01-01T00:00:00.000Z",
    updatedAt: "2026-01-01T00:00:00.000Z",
};

export const databaseId = "01900000-0000-7000-8000-000000000010";
export const schemaId = "f4077b65-9a42-5ade-b648-299c09235271";
const text = (value: string): SqlValue => ({ type: "text", value });
export class SqlTransport {
    records = new Map<string, unknown>();
    initialized = false;
    version = "1";
    tables: string[] = [];
    requests: { url: string; init: RequestInit; request?: SqlRequest }[] = [];
    ensureStatus = 200;
    readStatus = 200;
    writeStatus = 200;
    uncertain = false;
    malformed = false;
    async fetch(
        url: URL | RequestInfo,
        init: RequestInit = {},
    ): Promise<Response> {
        const path = String(url);
        if (path.endsWith("/oauth2/token")) {
            const body = new URLSearchParams(init.body as string);
            if (body.get("resource") !== "storage:databases")
                throw new Error("Wrong Storage audience");
            return Response.json({ access_token: "storage-token" });
        }
        this.requests.push({ url: path, init });
        if (
            new Headers(init.headers).get("Authorization") !==
            "Bearer storage-token"
        )
            throw new Error("Missing SQL authorization");
        if (path.endsWith("/databases/ensure"))
            return Response.json(
                this.ensureStatus === 200
                    ? { id: databaseId, name: "Password Manager" }
                    : { code: "redacted" },
                { status: this.ensureStatus },
            );
        if (!path.endsWith(`/databases/${databaseId}/sql`))
            throw new Error("Unexpected Storage operation");
        const request = JSON.parse(init.body as string) as SqlRequest;
        required(this.requests.at(-1)).request = request;
        const mutation = request.statements.some(({ sql }) =>
            /^(INSERT INTO secrets|UPDATE secrets|DELETE FROM secrets)/.test(
                sql,
            ),
        );
        if (mutation && this.writeStatus !== 200)
            return Response.json(
                { code: "executionFailed" },
                { status: this.writeStatus },
            );
        if (!mutation && this.readStatus !== 200)
            return Response.json(
                { code: "unavailable" },
                { status: this.readStatus },
            );
        const results = request.statements.map(({ sql, params = [] }) => {
            const values = params.map((p) => ("value" in p ? p.value : null));
            if (sql === "SELECT name FROM tables")
                return {
                    columns: ["name"],
                    rows: this.tables.map((name) => [text(name)]),
                };
            if (sql.startsWith("CREATE TABLE")) {
                this.initialized = true;
                const name = sql.includes("secret_schema")
                    ? "secret_schema"
                    : "secrets";
                if (!this.tables.includes(name)) this.tables.push(name);
            } else if (sql.startsWith("SELECT id, version")) {
                return {
                    columns: ["id", "version"],
                    rows: [
                        [
                            { type: "uuid", value: schemaId },
                            { type: "integer", value: this.version },
                        ],
                    ],
                };
            } else if (sql === "SELECT id, content FROM secrets") {
                return {
                    columns: ["id", "content"],
                    rows: [...this.records]
                        .map(([id, value]) => [
                            { type: "uuid", value: id },
                            text(JSON.stringify(value)),
                        ])
                        .concat(
                            this.malformed
                                ? [
                                      [
                                          {
                                              type: "uuid",
                                              value: "01900000-0000-7000-8000-000000000099",
                                          },
                                          text("broken private JSON"),
                                      ],
                                  ]
                                : [],
                        ),
                };
            } else if (sql.startsWith("INSERT INTO secrets")) {
                this.records.set(
                    String(values[0]),
                    JSON.parse(String(values[1])),
                );
            } else if (sql.startsWith("UPDATE secrets")) {
                if (this.records.has(String(values[1])))
                    this.records.set(
                        String(values[1]),
                        JSON.parse(String(values[0])),
                    );
            } else if (sql.startsWith("DELETE FROM secrets"))
                this.records.delete(String(values[0]));
            else if (!sql.startsWith("INSERT INTO secret_schema"))
                throw new Error(`Unexpected SQL: ${sql}`);
            return { columns: [], rows: [] };
        });
        if (mutation && this.uncertain)
            throw new Error("Transport lost: PRIVATE BODY MUST NOT LEAK");
        return Response.json({ results });
    }
    seed() {
        this.initialized = true;
        this.tables = ["secret_schema", "secrets"];
        this.records.set(id, structuredClone(secret));
    }
}
export function required<T>(value: T | null | undefined): T {
    if (value == null) throw new Error("Expected a populated test fixture");
    return value;
}
