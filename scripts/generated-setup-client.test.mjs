import assert from "node:assert/strict";
import test from "node:test";
import {
  Configuration,
  DefaultApi,
  BASE_PATH,
} from "../packages/idp-client/esm/setup/index.js";

const code = "a".repeat(64);
const intent = {
  operationNonce: "0123456789abcdef0123456789abcdef",
  administrator: "operator",
  node: "My Node",
};
const protectedOptions = {
  credentials: "omit",
  redirect: "error",
  cache: "no-store",
};

function client(fetchApi) {
  return new DefaultApi(
    new Configuration({
      basePath: "https://node.test",
      apiKey: `Setup ${code}`,
      fetchApi,
    }),
  );
}

test("generated setup defaults to HTTPS and sends exact accepted Create once", async () => {
  assert.equal(BASE_PATH, "https://localhost");
  let calls = 0;
  const api = client(async (url, options) => {
    calls++;
    assert.equal(url, "https://node.test/idp/setup/create");
    assert.equal(options.headers.Authorization, `Setup ${code}`);
    assert.equal(options.method, "POST");
    for (const [key, value] of Object.entries(protectedOptions))
      assert.equal(options[key], value);
    assert.deepEqual(JSON.parse(options.body), {
      operation_nonce: intent.operationNonce,
      administrator: "operator",
      node: "My Node",
      password: "protected input",
    });
    return Response.json(
      {
        stage: "installation",
        state: "creating",
        ready: false,
        owners: ["idp", "management", "storage"],
      },
      { status: 202 },
    );
  });
  const response = await api.createRaw(
    { createRequest: { ...intent, password: "protected input" } },
    protectedOptions,
  );
  assert.equal(response.raw.status, 202);
  assert.equal((await response.value()).ready, false);
  assert.equal(calls, 1);
});

test("generated recovery distinguishes exact intent from unbound 204", async () => {
  const api = client(async (url, options) => {
    assert.equal(url, "https://node.test/idp/setup/intent");
    assert.equal(options.headers.Authorization, `Setup ${code}`);
    assert.equal(options.body, undefined);
    return Response.json({
      operation_nonce: intent.operationNonce,
      administrator: intent.administrator,
      node: intent.node,
    });
  });
  assert.deepEqual(await api.intent(protectedOptions), intent);
  assert.equal(
    await client(async () => new Response(null, { status: 204 })).intent(
      protectedOptions,
    ),
    null,
  );
});

test("generated cancellation omits accidental password and preserves HTTP denial", async () => {
  await client(async (url, options) => {
    assert.equal(url, "https://node.test/idp/setup/cancel");
    assert.deepEqual(JSON.parse(options.body), {
      operation_nonce: intent.operationNonce,
      administrator: intent.administrator,
      node: intent.node,
    });
    return new Response(null, { status: 204 });
  }).cancel(
    { setupIntent: { ...intent, password: "never serialized" } },
    protectedOptions,
  );
  await assert.rejects(
    client(async () => new Response(null, { status: 401 })).admission(
      protectedOptions,
    ),
    (error) => error.response.status === 401,
  );
});

test("generated Join preserves exact request, recovered issuer and cancellation", async () => {
  const joining = {
    operationNonce: intent.operationNonce,
    node: "Joining Node",
    peer: "a".repeat(64),
  };
  const retained = { ...joining, issuer: "https://installation.test/idp" };
  const wire = {
    operation_nonce: joining.operationNonce,
    node: joining.node,
    peer: joining.peer,
    issuer: retained.issuer,
  };
  const api = client(async (url, options) => {
    assert.equal(options.headers.Authorization, `Setup ${code}`);
    if (url.endsWith("/join")) {
      const { issuer, ...request } = wire;
      assert.deepEqual(JSON.parse(options.body), request);
      return Response.json(
        {
          stage: "installation",
          state: "joining",
          ready: false,
          owners: ["idp", "management", "storage"],
        },
        { status: 202 },
      );
    }
    if (url.endsWith("/intent")) return Response.json(wire);
    assert.equal(url, "https://node.test/idp/setup/cancel");
    assert.deepEqual(JSON.parse(options.body), wire);
    return new Response(null, { status: 204 });
  });
  const accepted = await api.joinRaw(
    { joinRequest: joining },
    protectedOptions,
  );
  assert.equal(accepted.raw.status, 202);
  assert.equal((await accepted.value()).state, "joining");
  assert.deepEqual(await api.intent(protectedOptions), retained);
  await api.cancel(
    { setupIntent: { ...retained, password: "never serialized" } },
    protectedOptions,
  );
});

test("generated public status sends no setup capability and decodes public login Client", async () => {
  const status = await client(async (url, options) => {
    assert.equal(url, "https://node.test/idp/setup/status");
    assert.equal(options.headers.Authorization, undefined);
    return Response.json({
      stage: "installation",
      state: "ready",
      ready: true,
      owners: ["idp", "management", "storage"],
      login_client: {
        client_id: "01234567-89ab-4def-8123-456789abcdef",
        scopes: ["openid"],
      },
    });
  }).status(protectedOptions);
  assert.equal(
    status.loginClient.clientId,
    "01234567-89ab-4def-8123-456789abcdef",
  );
  assert.equal(status.ready, true);
});
