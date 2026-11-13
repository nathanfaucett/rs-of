import { expect, test } from "vitest";
import { StorageClient } from "../packages/storage-client/src/index";
import { backendURL, json, session } from "./browser-session";

test("all four owners are healthy in a real browser", async () => {
  for (const owner of ["idp", "device", "storage", "management"]) {
    expect((await fetch(`${backendURL}/${owner}/health`)).status).toBe(200);
  }
});

test("browser authenticates, reads Devices, and persists Storage writes", async () => {
  const login = await session();
  const user = await json(
    await fetch(`${backendURL}/idp/userinfo`, {
      headers: { Authorization: `Bearer ${login.access_token}` },
    })
  );
  expect(user.email).toBe("admin@local.test");
  const token = await json(
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
  const storage = new StorageClient({
    idpBaseUrl: `${backendURL}/idp`,
    storageBaseUrl: `${backendURL}/storage`,
    clientId: "local-login",
    bearerToken: () => login.access_token,
  });
  const resource = await storage.createDatabase(`browser-${crypto.randomUUID()}`);
  try {
    await storage.executeSql(
      resource.id,
      {
        statements: [
          { sql: "CREATE TABLE notes (value TEXT NOT NULL)", params: [] },
          {
            sql: "INSERT INTO notes (value) VALUES (?)",
            params: [{ type: "text", value: "browser write" }],
          },
        ],
      },
      ["write"]
    );
    const read = await storage.executeSql(
      resource.id,
      { statements: [{ sql: "SELECT value FROM notes", params: [] }] },
      ["read"]
    );
    expect(read.results[0].rows).toEqual([[{ type: "text", value: "browser write" }]]);
  } finally {
    await storage.deleteDatabase(resource.id);
  }
  expect((await storage.listDatabases()).map((entry) => entry.id)).not.toContain(resource.id);
});
