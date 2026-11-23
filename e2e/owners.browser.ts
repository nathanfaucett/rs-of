import { expect, test } from "vitest";
import {
  DatabaseOperationError,
  type SqlValue,
  StorageClient,
} from "../packages/storage-client/src/index";
import { backendURL, json, session } from "./browser-session";

test("all four owners are healthy in a real browser", async () => {
  for (const owner of ["idp", "device", "storage", "management"]) {
    expect((await fetch(`${backendURL}/${owner}/health`)).status).toBe(200);
  }
});

test("browser authenticates and reads real User info", async () => {
  const login = await session();
  const user = await json<{ email: string }>(
    await fetch(`${backendURL}/idp/userinfo`, {
      headers: { Authorization: `Bearer ${login.access_token}` },
    })
  );
  expect(user.email).toBe("admin@local.test");
});

test("browser exchanges its session for Device access", async () => {
  const login = await session();
  const token = await json<{ access_token: string }>(
    await fetch(`${backendURL}/idp/oauth2/token`, {
      method: "POST",
      body: new URLSearchParams({
        grant_type: "urn:ietf:params:oauth:grant-type:token-exchange",
        subject_token: login.access_token,
        subject_token_type: "urn:ietf:params:oauth:token-type:access_token",
        resource: `${backendURL}/device`,
        client_id: "local-login",
      }),
    })
  );
  const devices = await json(
    await fetch(`${backendURL}/device/devices`, {
      headers: { Authorization: `Bearer ${token.access_token}` },
    })
  );
  expect(devices).toEqual(expect.any(Array));
});

test("browser Storage SQL generates UUID primary keys and rejects supplied ones", async () => {
  const login = await session();
  const storage = new StorageClient({
    idpBaseUrl: `${backendURL}/idp`,
    storageBaseUrl: `${backendURL}/storage`,
    clientId: "local-login",
    bearerToken: () => login.access_token,
  });
  const resource = await storage.createDatabase(`browser-${crypto.randomUUID()}`);
  const execute = async (sql: string, params: SqlValue[] = []) => {
    const response = await storage.executeSql(resource.id, { statements: [{ sql, params }] }, [
      "read",
      "write",
    ]);
    const result = response.results[0];
    if (!result) throw new Error("Storage SQL execute returned no result");
    return result.rows;
  };
  const uuid5 = "018f0f8e-7b6d-5c4a-8f12-123456789abc";
  const uuid7 = "018f0f8e-7b6d-7c4a-8f12-123456789abc";
  const uuid = (value: string): SqlValue => ({ type: "uuid", value });
  try {
    await execute("CREATE TABLE notes (id UUID PRIMARY KEY, ref UUID NOT NULL)");
    const omitted = await execute("INSERT INTO notes (ref) VALUES (?) RETURNING id", [uuid(uuid5)]);
    const defaulted = await execute(
      "INSERT INTO notes (id, ref) VALUES (DEFAULT, ?) RETURNING id",
      [uuid(uuid7)]
    );
    const generated = {
      type: "uuid",
      value: expect.stringMatching(
        /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i
      ),
    };
    expect(omitted).toEqual([[generated]]);
    expect(defaulted).toEqual([[generated]]);
    expect(omitted).not.toEqual(defaulted);

    for (const id of [uuid5, uuid7]) {
      for (const insert of [
        () => execute(`INSERT INTO notes VALUES (CAST('${id}' AS UUID), ?)`, [uuid(id)]),
        () => execute("INSERT INTO notes VALUES (?, ?)", [uuid(id), uuid(id)]),
      ]) {
        const failure = insert();
        await expect(failure).rejects.toBeInstanceOf(DatabaseOperationError);
        await expect(failure).rejects.toMatchObject({ status: 400 });
      }
    }
    expect(await execute("SELECT id FROM notes WHERE ref = ?", [uuid(uuid5)])).toEqual(omitted);
    expect(await execute("SELECT id FROM notes WHERE ref = ?", [uuid(uuid7)])).toEqual(defaulted);

    const key: SqlValue = { type: "text", value: "browser" };
    await execute("CREATE TABLE natural_keys (name TEXT PRIMARY KEY, ref UUID)");
    await execute("CREATE TABLE composite_keys (name TEXT, ref UUID, PRIMARY KEY (name, ref))");
    for (const table of ["natural_keys", "composite_keys"]) {
      await execute(`INSERT INTO ${table} VALUES (?, ?)`, [key, uuid(uuid5)]);
      expect(await execute(`SELECT name, ref FROM ${table}`)).toEqual([[key, uuid(uuid5)]]);
    }
  } finally {
    await storage.deleteDatabase(resource.id);
  }
  expect((await storage.listDatabases()).map((entry) => entry.id)).not.toContain(resource.id);
});
