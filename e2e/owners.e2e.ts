import { generateKeyPairSync, sign } from "node:crypto";
import { expect, test } from "@playwright/test";
import { Configuration, DefaultApi } from "../packages/device-client/src/index";
import { appURL, backendURL, passwordManagerURL } from "./backend.mjs";
import { frontendSession, json, storageClient } from "./session";
import { checkedFetch } from "./http";

test("fresh backend exposes four healthy owners and real OIDC metadata", async ({ request }) => {
  for (const owner of ["idp", "device", "storage", "management"]) {
    expect((await request.get(`${backendURL}/${owner}/health`)).status()).toBe(200);
  }
  const metadata = await json(
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
]) {
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
      const read = () =>
        storage.executeSql(
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
      expect((await read()).results[0].rows).toEqual([[{ type: "text", value: "first" }]]);
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
      expect((await read()).results[0].rows).toEqual([[{ type: "text", value: "changed" }]]);
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
      expect((await read()).results[0].rows).toEqual([]);
    } finally {
      await storage.deleteDatabase(resource.id);
    }
    expect((await storage.listDatabases()).map((database) => database.id)).not.toContain(
      resource.id
    );
  });
}

test("Device owner requires authorized approval and proof of a second Device key", async ({
  request,
}) => {
  const session = await frontendSession(request, "app", appURL);
  const token = await json(
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
  const publicKey = Buffer.from(key.x!, "base64url").toString("hex");
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
  const secondKey = Buffer.from(
    second.publicKey.export({ format: "jwk" }).x!, "base64url"
  ).toString("hex");
  const headers = { Authorization: `Bearer ${token.access_token}` };
  const pairings = `${backendURL}/device/devices/pairings`;
  const pending = await json(await request.post(pairings, {
    headers,
    data: {
      name: "Second Device",
      publicKey: secondKey,
      acceptingPublicKey: publicKey,
      address: "127.0.0.1:23457",
    },
  }));
  expect(pending.state).toBe("pending");
  const { payload } = await json(
    await request.get(`${pairings}/${pending.id}/payload`, { headers })
  );
  const approve = `${pairings}/${pending.id}/approve`;
  const bytes = Buffer.from(payload);
  const signature = sign(null, bytes, second.privateKey).toString("base64url");
  expect((await request.post(approve, { data: { signature } })).status()).toBe(401);
  expect((await request.post(approve, { headers, data: { signature: "" } })).status()).toBe(403);
  const wrongProof = sign(null, bytes, first.privateKey).toString("base64url");
  expect((await request.post(approve, { headers, data: { signature: wrongProof } })).status()).toBe(403);
  const unsignedApproval = await request.post(
    `${backendURL}/device/devices/enrollments/${pending.id}/approve`, { headers }
  );
  expect(unsignedApproval.ok()).toBe(false);
  const approved = await json(await request.post(approve, { headers, data: { signature } }));
  expect(approved).toMatchObject({ id: pending.id, publicKey: secondKey, state: "approved" });
  expect((await request.post(approve, { headers, data: { signature } })).status()).toBe(403);
  const denied = await request.get(`${backendURL}/device/devices`);
  expect(denied.ok()).toBeFalsy();
});
