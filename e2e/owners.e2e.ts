import { generateKeyPairSync, sign } from "node:crypto";
import { expect, test } from "@playwright/test";
import {
  Configuration,
  DefaultApi,
  type DeviceEnrollment,
  type DeviceInfo,
  type DevicePairingApprovalPayload,
} from "../packages/device-client/src/index";
import { appURL, backendURL, passwordManagerURL } from "./backend.mjs";
import { checkedFetch } from "./http";
import { frontendSession, json, passwordSession, signIn, storageClient } from "./session";

test("fresh backend exposes four healthy owners and real OIDC metadata", async ({ request }) => {
  for (const owner of ["idp", "device", "storage", "management"]) {
    expect((await request.get(`${backendURL}/${owner}/health`)).status()).toBe(200);
  }
  const metadata = await json<{ issuer: string; token_endpoint: string }>(
    await request.get(`${backendURL}/idp/.well-known/openid-configuration`)
  );
  expect(metadata.issuer).toBe(`${backendURL}/idp`);
  expect(metadata.token_endpoint).toBe(`${backendURL}/idp/oauth2/token`);
  const denied = await request.post(`${backendURL}/idp/oauth2/token`, {
    form: {
      grant_type: "password",
      client_id: "local-login",
      username: "admin@local.test",
      password: "wrong-password",
    },
  });
  expect(denied.ok()).toBeFalsy();
});

for (const [clientId, origin] of [
  ["app", appURL],
  ["password-manager", passwordManagerURL],
] as const) {
  test(`${clientId}: sign-in, consent and PKCE establish a real User session`, async ({
    request,
  }) => {
    await frontendSession(request, clientId, origin);
  });

  test(`${clientId}: PKCE consent session can create, read, update and delete Storage data`, async ({
    request,
  }) => {
    const session = await frontendSession(request, clientId, origin);
    const storage = storageClient(clientId, session.access_token);
    const resource = await storage.createDatabase(`e2e-${clientId}`);
    try {
      expect((await storage.listDatabases()).map((database) => database.id)).toContain(resource.id);
      await storage.executeSql(
        resource.id,
        {
          statements: [
            {
              sql: "CREATE TABLE e2e_notes (id TEXT PRIMARY KEY, value TEXT NOT NULL)",
              params: [],
            },
            {
              sql: "INSERT INTO e2e_notes (id, value) VALUES (?, ?)",
              params: [
                { type: "text", value: "note" },
                { type: "text", value: "first" },
              ],
            },
          ],
        },
        ["write"]
      );
      const read = async () => {
        const response = await storage.executeSql(
          resource.id,
          {
            statements: [
              {
                sql: "SELECT value FROM e2e_notes WHERE id = ?",
                params: [{ type: "text", value: "note" }],
              },
            ],
          },
          ["read"]
        );
        const result = response.results[0];
        if (!result) throw new Error("Storage SQL read returned no result");
        return result.rows;
      };
      expect(await read()).toEqual([[{ type: "text", value: "first" }]]);
      await storage.executeSql(
        resource.id,
        {
          statements: [
            {
              sql: "UPDATE e2e_notes SET value = ? WHERE id = ?",
              params: [
                { type: "text", value: "changed" },
                { type: "text", value: "note" },
              ],
            },
          ],
        },
        ["write"]
      );
      expect(await read()).toEqual([[{ type: "text", value: "changed" }]]);
      await storage.executeSql(
        resource.id,
        {
          statements: [
            {
              sql: "DELETE FROM e2e_notes WHERE id = ?",
              params: [{ type: "text", value: "note" }],
            },
          ],
        },
        ["write"]
      );
      expect(await read()).toEqual([]);
    } finally {
      await storage.deleteDatabase(resource.id);
    }
    expect((await storage.listDatabases()).map((database) => database.id)).not.toContain(
      resource.id
    );
  });
}

test("Device owner requires signed approval and the app can find and sort the approved Devices", async ({
  page,
  request,
}) => {
  const session = await frontendSession(request, "app", appURL);
  const token = await json<{ access_token: string }>(
    await request.post(`${backendURL}/idp/oauth2/token`, {
      form: {
        grant_type: "urn:ietf:params:oauth:grant-type:token-exchange",
        subject_token: session.access_token,
        subject_token_type: "urn:ietf:params:oauth:token-type:access_token",
        resource: `${backendURL}/device`,
        client_id: "app",
      },
    })
  );
  const device = new DefaultApi(
    new Configuration({
      basePath: `${backendURL}/device`,
      accessToken: token.access_token,
      fetchApi: checkedFetch,
    })
  );
  const first = generateKeyPairSync("ed25519");
  const key = first.publicKey.export({
    format: "jwk",
  });
  if (!key.x) throw new Error("Ed25519 public key is missing its x coordinate");
  const publicKey = Buffer.from(key.x, "base64url").toString("hex");
  const enrollment = await device.enrollDevice({
    deviceEnrollmentRequest: {
      name: "E2E browser device",
      publicKey,
      address: "127.0.0.1:23456",
    },
  });
  expect(enrollment.id).toEqual(expect.any(String));
  expect(enrollment.state).toBe("approved");
  expect(await device.listDevices()).toEqual(
    expect.arrayContaining([
      expect.objectContaining({
        id: enrollment.id,
        name: "E2E browser device",
      }),
    ])
  );
  const second = generateKeyPairSync("ed25519");
  const secondJwk = second.publicKey.export({ format: "jwk" });
  if (!secondJwk.x) throw new Error("Second Ed25519 public key is missing its x coordinate");
  const secondKey = Buffer.from(secondJwk.x, "base64url").toString("hex");
  const headers = { Authorization: `Bearer ${token.access_token}` };
  const pairings = `${backendURL}/device/devices/pairings`;
  const pending = await json<DeviceEnrollment>(
    await request.post(pairings, {
      headers,
      data: {
        name: "Second Device",
        publicKey: secondKey,
        acceptingPublicKey: publicKey,
        address: "127.0.0.1:23457",
      },
    })
  );
  expect(pending.state).toBe("pending");
  const { payload } = await json<DevicePairingApprovalPayload>(
    await request.get(`${pairings}/${pending.id}/payload`, { headers })
  );
  const approve = `${pairings}/${pending.id}/approve`;
  expect(typeof payload).toBe("string");
  const bytes = Buffer.from(payload, "utf8");
  const signature = sign(null, bytes, second.privateKey).toString("base64url");
  const pendingEnrollments = async () =>
    json<DeviceEnrollment[]>(
      await request.get(`${backendURL}/device/devices/enrollments`, { headers })
    );
  const before = await pendingEnrollments();
  expect(before).toContainEqual(pending);
  const wrongProof = sign(null, bytes, first.privateKey).toString("base64url");
  for (const [options, status] of [
    [{ data: { signature } }, 401],
    [{ headers, data: {} }, 422],
    [{ headers, data: { signature: "" } }, 403],
    [{ headers, data: { signature: wrongProof } }, 403],
  ] as const) {
    expect((await request.post(approve, options)).status()).toBe(status);
    expect(await pendingEnrollments()).toEqual(before);
  }
  const unsignedApproval = await request.post(
    `${backendURL}/device/devices/enrollments/${pending.id}/approve`,
    { headers }
  );
  expect(unsignedApproval.status()).toBe(404);
  expect(await pendingEnrollments()).toEqual(before);
  const approved = await json<DeviceInfo>(
    await request.post(approve, { headers, data: { signature } })
  );
  expect(approved).toMatchObject({ id: pending.id, publicKey: secondKey, state: "approved" });
  expect((await pendingEnrollments()).some(({ id }) => id === pending.id)).toBe(false);
  expect((await request.post(approve, { headers, data: { signature } })).status()).toBe(403);
  expect((await device.listDevices()).find(({ id }) => id === pending.id)).toEqual(approved);
  const denied = await request.get(`${backendURL}/device/devices`);
  expect(denied.ok()).toBeFalsy();

  // Use the two real Device records from this scenario, without adding another enrollment.
  await page.goto(`${appURL}/devices`);
  await signIn(page);
  const search = page.getByRole("searchbox", { name: "Search loaded devices" });
  const sort = page.getByRole("combobox", { name: "Sort loaded devices" });
  const records = page.locator("tbody tr").filter({ hasText: /E2E browser device|Second Device/ });
  await expect(records).toHaveCount(2);
  await search.fill(secondKey.toUpperCase());
  await expect(records).toHaveCount(1);
  await expect(records.first()).toContainText("Second Device");
  await search.fill("no matching device");
  await expect(page.getByText("No matching devices.")).toBeVisible();
  await search.fill("");
  await sort.selectOption("name-desc");
  await expect(records.first()).toContainText("Second Device");
  await sort.selectOption("name-asc");
  await expect(records.first()).toContainText("E2E browser device");
});

test("IdP Client read requires a token from the granted Application", async ({ request }) => {
  const sessions = [await passwordSession(request), await frontendSession(request, "app", appURL)];
  const statuses = [];
  for (const session of sessions) {
    const token = await json<{ access_token: string }>(
      await request.post(`${backendURL}/idp/oauth2/token`, {
        form: {
          grant_type: "urn:ietf:params:oauth:grant-type:token-exchange",
          subject_token: session.access_token,
          subject_token_type: "urn:ietf:params:oauth:token-type:access_token",
          resource: `${backendURL}/idp`,
        },
      })
    );
    const response = await request.get(`${backendURL}/idp/oauth2/register/local-login`, {
      headers: { Authorization: `Bearer ${token.access_token}` },
    });
    statuses.push(response.status());
  }
  expect(statuses).toEqual([200, 403]);
});
