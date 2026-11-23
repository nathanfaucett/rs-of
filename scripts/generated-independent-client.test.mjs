import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  Configuration,
  OwnerSetupApi as DefaultApi,
  IndependentCreateIntentToJSON,
  IndependentCreateRequestToJSON,
} from "../packages/idp-client/esm/index.js";

const operation = {
  purpose: "create",
  operationNonce: "1".repeat(32),
  administrator: "Alice",
  node: "New Node",
  idp: "https://target.test/idp",
  device: "https://target.test/device",
  storage: "https://target.test/storage",
  issuer: "https://target.test/idp",
};
const options = { credentials: "omit", redirect: "error", cache: "no-store" };

test("complete owner setup schema includes worker handoffs with owner-relative bases", () => {
  const document = JSON.parse(readFileSync(new URL("../openapi/idp.json", import.meta.url)));
  assert.deepEqual(
    Object.keys(document.paths)
      .filter((path) => path.startsWith("/setup/owner/"))
      .sort(),
    [
      "/setup/owner/admission",
      "/setup/owner/cancel",
      "/setup/owner/challenge",
      "/setup/owner/create",
      "/setup/owner/intent",
      "/setup/owner/public",
      "/setup/owner/register",
      "/setup/owner/status",
    ]
  );
});
test("generated Create sends the exact owner tuple and password only to IdP", async () => {
  for (const owner of ["idp", "device", "storage"]) {
    const api = new DefaultApi(
      new Configuration({
        basePath: operation[owner],
        apiKey: `Setup ${owner}`,
        fetchApi: async (url, init) => {
          assert.equal(url, `${operation[owner]}/setup/owner/create`);
          assert.equal(init.headers.Authorization, `Setup ${owner}`);
          const { operationNonce, ...fields } = operation;
          assert.deepEqual(JSON.parse(init.body), {
            ...fields,
            operation_nonce: operationNonce,
            ...(owner === "idp" ? { password: "protected-password" } : {}),
          });
          return new Response(null, { status: 202 });
        },
      })
    );
    const raw = await api.ownerSetupCreateRaw(
      {
        independentCreateRequest:
          owner === "idp" ? { ...operation, password: "protected-password" } : operation,
      },
      options
    );
    assert.equal(raw.raw.status, 202);
  }
});
test("generated serializers preserve the wire tuple and optional IdP-only password", () => {
  const create = operation;
  const intent = JSON.parse(JSON.stringify(IndependentCreateIntentToJSON(create)));
  assert.equal(intent.operation_nonce, operation.operationNonce);
  assert.ok(!("password" in intent));
  assert.ok(!("operationNonce" in intent));
  assert.equal(
    JSON.parse(
      JSON.stringify(
        IndependentCreateRequestToJSON({
          ...create,
          password: "protected-password",
        })
      )
    ).password,
    "protected-password"
  );
  assert.ok(!("password" in JSON.parse(JSON.stringify(IndependentCreateRequestToJSON(create)))));
});
test("public observation carries no setup code and recovery/cancel retain distinct status semantics", async () => {
  const calls = [];
  const api = new DefaultApi(
    new Configuration({
      basePath: operation.device,
      apiKey: "Setup code",
      fetchApi: async (url, init) => {
        calls.push({ url, init });
        if (url.endsWith("/status"))
          return Response.json({
            stage: "installation",
            state: "creating",
            ready: false,
            owners: ["device"],
          });
        if (url.endsWith("/public"))
          return Response.json({
            owner: "device",
            operation: IndependentCreateIntentToJSON(operation),
            clients: [],
            facts: {},
            registered: [],
          });
        return new Response(null, { status: 204 });
      },
    })
  );
  await api.ownerSetupStatus(options);
  await api.ownerSetupOwnerPublic(options);
  for (const { init } of calls) assert.equal(init.headers.Authorization, undefined);
  assert.equal((await api.ownerSetupRecoverRaw(options)).raw.status, 204);
  assert.equal(calls.at(-1).init.body, undefined);
  assert.equal(calls.at(-1).init.headers.Authorization, "Setup code");
  assert.equal(
    (
      await api.ownerSetupCancelRaw(
        {
          independentCreateIntent: {
            ...operation,
            issuer: "https://installation.test/idp",
          },
        },
        options
      )
    ).raw.status,
    204
  );
  assert.ok(!("password" in JSON.parse(calls.at(-1).init.body)));
});
