import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { fetchSetupStage, verifySetupCode } from "./setupApi.ts";
import * as setup from "./setupApi.ts";

test("authorized recovery reads exact retained Create without sending or persisting secrets", async () => {
  globalThis.fetch = async (url, options) => {
    assert.equal(url, "https://node.test/idp/setup/intent");
    assert.equal(options.method, "POST");
    assert.equal(options.body, undefined);
    assert.equal(options.credentials, "omit");
    assert.equal(options.redirect, "error");
    assert.equal(options.cache, "no-store");
    assert.deepEqual(options.headers, {
      Authorization: `Setup ${"a".repeat(64)}`,
    });
    return Response.json(intent);
  };
  assert.deepEqual(
    await setup.fetchSetupIntent("https://node.test/idp", "a".repeat(64)),
    intent,
  );
  globalThis.fetch = async () => new Response(null, { status: 204 });
  assert.equal(
    await setup.fetchSetupIntent("https://node.test/idp", "a".repeat(64)),
    null,
  );
});

test("recovery rejects malformed intent and denial without echoing protected data", async () => {
  for (const value of [
    null,
    [],
    {},
    { ...intent, operation_nonce: "invalid" },
    { ...intent, administrator: " admin" },
    { ...intent, password: "protected password" },
  ]) {
    globalThis.fetch = async () => Response.json(value);
    await assert.rejects(
      setup.fetchSetupIntent("https://node.test/idp", "a".repeat(64)),
      /invalid/i,
    );
  }
  for (const status of [201, 401, 403, 503]) {
    globalThis.fetch = async () =>
      new Response("protected password", { status });
    await assert.rejects(
      setup.fetchSetupIntent("https://node.test/idp", "a".repeat(64)),
      (error) =>
        error.message.includes(String(status)) &&
        !error.message.includes("protected password"),
    );
  }
  globalThis.fetch = async () => {
    throw new Error("protected password");
  };
  await assert.rejects(
    setup.fetchSetupIntent("https://node.test/idp", "a".repeat(64)),
    (error) => !error.message.includes("protected password"),
  );
});

test("Join admits only nonce/node/peer, recovers issuer and cancels the exact authorized intent", async () => {
  const request = {
    operation_nonce: "1".repeat(32),
    node: "Joined Node",
    peer: "a".repeat(64),
  };
  const retained = { ...request, issuer: "https://installation.test" };
  const joining = { ...creating, state: "joining" };
  globalThis.fetch = async (url, options) => {
    assert.equal(url, "https://node.test/idp/setup/join");
    assert.deepEqual(JSON.parse(options.body), request);
    assert.equal(options.headers.Authorization, `Setup ${"a".repeat(64)}`);
    assert.equal(options.credentials, "omit");
    assert.equal(options.redirect, "error");
    return Response.json(joining, { status: 202 });
  };
  assert.deepEqual(
    await setup.submitSetupJoin(
      "https://node.test/idp",
      "a".repeat(64),
      request,
    ),
    joining,
  );
  globalThis.fetch = async () => Response.json(retained);
  assert.deepEqual(
    await setup.fetchSetupIntent("https://node.test/idp", "a".repeat(64)),
    retained,
  );
  globalThis.fetch = async (url, options) => {
    assert.equal(url, "https://node.test/idp/setup/cancel");
    assert.deepEqual(JSON.parse(options.body), retained);
    return new Response(null, { status: 204 });
  };
  await setup.cancelSetupJoin(
    "https://node.test/idp",
    "a".repeat(64),
    retained,
  );
});

const originalFetch = globalThis.fetch;
const scope = ["idp", "management", "storage"];
const intent = {
  operation_nonce: "0123456789abcdef0123456789abcdef",
  administrator: "operator",
  node: "My Node",
};
const creating = {
  stage: "installation",
  state: "creating",
  ready: false,
  owners: scope,
};
const ready = {
  stage: "installation",
  state: "ready",
  ready: true,
  owners: scope,
  login_client: {
    client_id: "01234567-89ab-4def-8123-456789abcdef",
    scopes: ["openid"],
  },
};

test("web observes host-owned Create and verified readiness, not just awaiting choice", async () => {
  for (const state of ["creating", "unfinished"]) {
    globalThis.fetch = async () => Response.json({ ...creating, state });
    assert.deepEqual(await setup.fetchSetupStatus("https://node.test/idp"), {
      ...creating,
      state,
    });
  }
  globalThis.fetch = async () => Response.json(ready);
  assert.deepEqual(await setup.fetchSetupStatus("https://node.test/idp"), {
    ...ready,
    stage: "ready",
  });
});

test("independent readiness requires login evidence only from IdP", async () => {
  for (const owner of ["management", "storage"]) {
    const status = { ...ready, owners: [owner] };
    delete status.login_client;
    globalThis.fetch = async () => Response.json(status);
    assert.deepEqual(await setup.fetchSetupStatus(`https://${owner}.test`), {
      ...status,
      stage: "ready",
    });
    globalThis.fetch = async () => Response.json({ ...status, login_client: ready.login_client });
    await assert.rejects(setup.fetchSetupStatus(`https://${owner}.test`), /invalid/i);
  }
  for (const owners of [["idp"], scope]) {
    const status = { ...ready, owners };
    delete status.login_client;
    globalThis.fetch = async () => Response.json(status);
    await assert.rejects(setup.fetchSetupStatus("https://idp.test"), /invalid/i);
  }
});

test("Create sends exact transient intent and admission then returns acceptance without polling", async () => {
  let calls = 0;
  const code = "a".repeat(64);
  const request = { ...intent, password: "protected password" };
  globalThis.fetch = async (url, options) => {
    calls++;
    assert.equal(url, "https://node.test/idp/setup/create");
    assert.equal(options.method, "POST");
    assert.equal(options.credentials, "omit");
    assert.equal(options.redirect, "error");
    assert.equal(options.cache, "no-store");
    assert.equal(
      new Headers(options.headers).get("Authorization"),
      `Setup ${code}`,
    );
    assert.equal(
      new Headers(options.headers).get("Content-Type"),
      "application/json",
    );
    assert.deepEqual(JSON.parse(options.body), request);
    return Response.json(creating, { status: 202 });
  };
  assert.deepEqual(
    await setup.submitSetupCreate("https://node.test/idp", code, request),
    creating,
  );
  assert.equal(calls, 1);
});

test("Create refuses malformed acceptance, owner-only responses and remote secret errors", async () => {
  for (const [status, value] of [
    [202, ready],
    [200, creating],
    [201, creating],
    [202, { ...creating, owners: ["idp"] }],
    [202, { ...creating, state: "unfinished" }],
  ]) {
    globalThis.fetch = async () => Response.json(value, { status });
    await assert.rejects(
      setup.submitSetupCreate("https://node.test/idp", "a".repeat(64), {
        ...intent,
        password: "protected password",
      }),
    );
  }
  globalThis.fetch = async () =>
    new Response("protected password", { status: 409 });
  await assert.rejects(
    setup.submitSetupCreate("https://node.test/idp", "a".repeat(64), {
      ...intent,
      password: "protected password",
    }),
    (error) =>
      error.message.includes("409") &&
      !error.message.includes("protected password"),
  );
  globalThis.fetch = async () => {
    throw new Error("protected password");
  };
  await assert.rejects(
    setup.submitSetupCreate("https://node.test/idp", "a".repeat(64), {
      ...intent,
      password: "protected password",
    }),
    (error) =>
      error.message.includes("host may still be working") &&
      !error.message.includes("protected password"),
  );
});

test("Join refuses malformed acceptance, retained issuer and pinned peer without trusting response bodies", async () => {
  const request = {
    operation_nonce: "1".repeat(32),
    node: "Joined Node",
    peer: "b".repeat(64),
  };
  for (const [status, value] of [
    [200, ready],
    [202, ready],
    [202, creating],
    [202, { ...creating, state: "joining", owners: ["idp"] }],
    [202, { ...creating, state: "unfinished" }],
    [409, { password: "secret" }],
  ]) {
    globalThis.fetch = async () => Response.json(value, { status });
    await assert.rejects(
      setup.submitSetupJoin("https://node.test/idp", "a".repeat(64), request),
      (error) => !error.message.includes("secret"),
    );
  }
  for (const retained of [
    request,
    { ...request, issuer: "http://installation.test" },
    { ...request, issuer: "https://installation.test?secret=code" },
    { ...request, issuer: "https://user:password@installation.test" },
    { ...request, issuer: "https://installation.test", password: "secret" },
    { ...request, issuer: "https://installation.test", peer: "B".repeat(64) },
  ]) {
    globalThis.fetch = async () => Response.json(retained);
    await assert.rejects(
      setup.fetchSetupIntent("https://node.test/idp", "a".repeat(64)),
      /Invalid/,
    );
  }
  globalThis.fetch = async () =>
    assert.fail("invalid Join must not send a request");
  for (const invalid of [
    { ...request, node: " Joined Node" },
    { ...request, peer: "0".repeat(64) },
    { ...request, peer: "b".repeat(63) },
  ])
    await assert.rejects(
      setup.submitSetupJoin("https://node.test/idp", "a".repeat(64), invalid),
    );
  await assert.rejects(
    setup.cancelSetupJoin("https://node.test/idp", "a".repeat(64), request),
  );
});

test("ready observation requires complete consistent public login evidence", async () => {
  for (const value of [
    { ...ready, login_client: undefined },
    { ...ready, ready: false },
    {
      ...ready,
      login_client: { client_id: "idp-web", scopes: ["openid"] },
    },
    {
      ...ready,
      login_client: {
        client_id: ready.login_client.client_id,
        scopes: [],
      },
    },
    {
      ...ready,
      login_client: {
        client_id: ready.login_client.client_id,
        scopes: ["openid", "openid"],
      },
    },
    { ...creating, login_client: ready.login_client },
  ]) {
    globalThis.fetch = async () => Response.json(value);
    await assert.rejects(
      setup.fetchSetupStatus("https://node.test/idp"),
      /invalid/,
    );
  }
});

test("invalid Create intent or password cannot send a privileged request", async () => {
  globalThis.fetch = async () =>
    assert.fail("invalid request must not be sent");
  for (const request of [
    {
      ...intent,
      operation_nonce: "not a retained nonce",
      password: "password",
    },
    { ...intent, administrator: " operator", password: "password" },
    { ...intent, node: "", password: "password" },
    { ...intent, node: "é".repeat(65), password: "password" },
    { ...intent, password: "" },
    { ...intent, password: "p\nassword" },
    { ...intent, password: "é".repeat(513) },
  ])
    await assert.rejects(
      setup.submitSetupCreate("https://node.test/idp", "a".repeat(64), request),
      /Invalid/,
    );
});

test("cancellation sends only exact nonsecret intent; 204 is not completion", async () => {
  globalThis.fetch = async (url, options) => {
    assert.equal(url, "https://node.test/idp/setup/cancel");
    assert.deepEqual(JSON.parse(options.body), intent);
    assert.equal(
      new Headers(options.headers).get("Authorization"),
      `Setup ${"a".repeat(64)}`,
    );
    return new Response(null, { status: 204 });
  };
  assert.equal(
    await setup.cancelSetupCreate("https://node.test/idp", "a".repeat(64), {
      ...intent,
      password: "must not leak",
    }),
    undefined,
  );
});
afterEach(() => {
  globalThis.fetch = originalFetch;
});

test("web setup reads the configured owner status without cookies or native IPC", async () => {
  globalThis.fetch = async (url, options) => {
    assert.equal(url, "https://node.test/idp/setup/status");
    assert.equal(options.credentials, "omit");
    assert.equal(options.redirect, "error");
    assert.equal(options.cache, "no-store");
    return Response.json({
      stage: "installation",
      state: "awaiting_choice",
      ready: false,
      owners: ["idp", "management", "storage"],
    });
  };
  assert.equal(await fetchSetupStage("https://node.test/idp/"), "installation");
});

test("missing or malformed setup evidence never manufactures readiness", async () => {
  for (const value of [
    {},
    { stage: "ready" },
    { stage: "ready", ready: false },
    { stage: "installation", ready: true },
    { stage: "unknown", ready: false },
  ]) {
    globalThis.fetch = async () => Response.json(value);
    await assert.rejects(fetchSetupStage("https://node.test/idp"), /invalid/i);
  }
  globalThis.fetch = async () => new Response(null, { status: 404 });
  await assert.rejects(fetchSetupStage("https://node.test/idp"), /404/);
});

test("web code verification uses an explicit header, never a URL or cookie", async () => {
  const code = "a".repeat(64);
  globalThis.fetch = async (url, options) => {
    assert.equal(url, "https://node.test/idp/setup/admission");
    assert.equal(options.method, "POST");
    assert.equal(options.credentials, "omit");
    assert.equal(options.redirect, "error");
    assert.equal(
      new Headers(options.headers).get("Authorization"),
      `Setup ${code}`,
    );
    assert.equal(options.body, undefined);
    return new Response(null, { status: 204 });
  };
  await verifySetupCode("https://node.test/idp", code);
});

test("admission errors never echo remote bodies or the code", async () => {
  const code = "a".repeat(64);
  globalThis.fetch = async () => new Response(code, { status: 401 });
  await assert.rejects(
    verifySetupCode("https://node.test/idp", code),
    (error) => {
      assert.equal(error.message, "Setup admission rejected (401)");
      return true;
    },
  );
  globalThis.fetch = async () => new Response(null, { status: 200 });
  await assert.rejects(verifySetupCode("https://node.test/idp", code), /200/);
});

test("invalid codes and insecure or ambiguous service bases are refused before a request", async () => {
  globalThis.fetch = async () => {
    assert.fail("invalid request must not be sent");
  };
  for (const base of [
    "",
    "http://node.test/idp",
    "https://user:password@node.test/idp",
    "https://node.test/idp?code=secret",
    "https://node.test/idp#fragment",
  ]) {
    await assert.rejects(fetchSetupStage(base), /HTTPS service base/i);
  }
  for (const code of [
    "",
    "a".repeat(63),
    "A".repeat(64),
    `${"a".repeat(64)}\n`,
  ]) {
    await assert.rejects(
      verifySetupCode("https://node.test/idp", code),
      /setup code/i,
    );
  }
});

test("unified scope yields one code target at the configured IdP base only", async () => {
  const calls = [];
  globalThis.fetch = async (url) => {
    calls.push(url);
    return Response.json({
      stage: "installation",
      state: "awaiting_choice",
      ready: false,
      owners: ["storage", "idp", "management"],
    });
  };
  const hosts = await setup.fetchSetupHosts({
    idp: "https://node.test/custom/",
  });
  assert.deepEqual(
    hosts.map(({ base, status }) => [base, status.owners]),
    [["https://node.test/custom", ["idp", "management", "storage"]]],
  );
  assert.deepEqual(calls, ["https://node.test/custom/setup/status"]);
});

test("independent owners each use their configured base and their own code, even on one origin", async () => {
  const bases = {
    idp: "https://node.test/auth",
    management: "https://node.test/policy",
    storage: "https://data.test/files",
  };
  const codes = ["a".repeat(64), "b".repeat(64), "c".repeat(64)];
  const calls = [];
  globalThis.fetch = async (url, options) => {
    calls.push([url, options]);
    const index = Object.values(bases).findIndex((base) =>
      url.startsWith(`${base}/setup/`),
    );
    assert.notEqual(index, -1);
    if (options.method === "POST") {
      assert.deepEqual(options.headers, {
        Authorization: `Setup ${codes[index]}`,
      });
      assert.equal(options.credentials, "omit");
      assert.equal(options.redirect, "error");
      assert.equal(options.cache, "no-store");
      assert.equal(options.body, undefined);
      return new Response(null, { status: 204 });
    }
    return Response.json({
      stage: "installation",
      state: "awaiting_choice",
      ready: false,
      owners: [Object.keys(bases)[index]],
    });
  };
  const hosts = await setup.fetchSetupHosts(bases);
  assert.deepEqual(
    hosts.map(({ base }) => base),
    Object.values(bases),
  );
  for (const [index, host] of hosts.entries())
    await verifySetupCode(host.base, codes[index]);
  assert.deepEqual(
    calls.map(([url]) => url),
    [
      ...Object.values(bases).map((base) => `${base}/setup/status`),
      ...Object.values(bases).map((base) => `${base}/setup/admission`),
    ],
  );
});

test("missing, duplicate, partial, unknown scopes and fabricated readiness fail closed", async () => {
  for (const owners of [
    undefined,
    null,
    [],
    ["idp", "storage"],
    ["idp", "idp", "storage"],
    ["unified"],
    ["IDP"],
    "idp",
    ["idp", "management", "other"],
  ]) {
    globalThis.fetch = async () =>
      Response.json({
        stage: "installation",
        state: "awaiting_choice",
        ready: false,
        owners,
      });
    await assert.rejects(
      setup.fetchSetupHosts({ idp: "https://node.test/idp" }),
      /invalid/,
    );
  }
  for (const value of [
    {
      stage: "ready",
      state: "awaiting_choice",
      ready: true,
      owners: ["idp"],
    },
    { stage: "installation", ready: false, owners: ["idp"] },
    {
      stage: "installation",
      state: "unknown",
      ready: false,
      owners: ["idp"],
    },
  ]) {
    globalThis.fetch = async () => Response.json(value);
    await assert.rejects(
      setup.fetchSetupStatus("https://node.test/idp"),
      /invalid/,
    );
  }
});

test("missing or duplicate independent bases and wrong owner status deny before admission", async () => {
  const bases = {
    idp: "https://node.test/idp",
    management: "https://node.test/management",
    storage: "https://store.test",
  };
  for (const config of [
    { idp: bases.idp },
    { ...bases, storage: undefined },
    { ...bases, management: `${bases.idp}/` },
    { ...bases, storage: bases.management },
  ]) {
    let calls = 0;
    globalThis.fetch = async () => {
      calls++;
      return Response.json({
        stage: "installation",
        state: "awaiting_choice",
        ready: false,
        owners: ["idp"],
      });
    };
    await assert.rejects(setup.fetchSetupHosts(config), /bases/);
    assert.equal(calls, 1);
  }
  for (const scope of [
    ["idp"],
    ["storage"],
    ["idp", "management", "storage"],
  ]) {
    globalThis.fetch = async (url, options) => {
      assert.equal(options.method, undefined);
      return Response.json({
        stage: "installation",
        state: "awaiting_choice",
        ready: false,
        owners: url === `${bases.idp}/setup/status` ? ["idp"] : scope,
      });
    };
    await assert.rejects(setup.fetchSetupHosts(bases), /does not match/);
  }
  globalThis.fetch = async () =>
    Response.json({
      stage: "installation",
      state: "awaiting_choice",
      ready: false,
      owners: ["management"],
    });
  await assert.rejects(setup.fetchSetupHosts(bases), /Primary setup scope/);
});

test("503 health loss retains explicit scope but never reports readiness", async () => {
  globalThis.fetch = async () =>
    Response.json(
      {
        stage: "installation",
        state: "unavailable",
        ready: false,
        owners: ["idp", "management", "storage"],
      },
      { status: 503 },
    );
  const hosts = await setup.fetchSetupHosts({ idp: "https://node.test/idp" });
  assert.deepEqual(hosts[0].status, {
    stage: "unavailable",
    state: "unavailable",
    ready: false,
    owners: ["idp", "management", "storage"],
  });
  globalThis.fetch = async () =>
    Response.json(
      {
        stage: "installation",
        state: "awaiting_choice",
        ready: false,
        owners: ["idp"],
      },
      { status: 503 },
    );
  await assert.rejects(
    setup.fetchSetupStatus("https://node.test/idp"),
    /invalid/,
  );
});

test("transport, JSON and HTTP failures never echo remote data or codes", async () => {
  const secret = "a".repeat(64);
  globalThis.fetch = async () => {
    throw new Error(secret);
  };
  await assert.rejects(
    verifySetupCode("https://node.test/idp", secret),
    (error) => !error.message.includes(secret),
  );
  await assert.rejects(
    setup.fetchSetupHosts({ idp: "https://node.test/idp" }),
    (error) => !error.message.includes(secret),
  );
  globalThis.fetch = async () => new Response(secret);
  await assert.rejects(
    setup.fetchSetupStatus("https://node.test/idp"),
    /Setup evidence is invalid/,
  );
  globalThis.fetch = async () => new Response(secret, { status: 403 });
  await assert.rejects(
    setup.fetchSetupStatus("https://node.test/idp"),
    (error) => error.message === "Setup status unavailable (403)",
  );
});

test("setup requires an exact current HTTPS UI origin, not a configured URL or a trust bypass", () => {
  setup.requireSetupUiOrigin("https://app.test:444");
  for (const origin of [
    "",
    "null",
    "http://app.test",
    "https://app.test/",
    "https://user@ app.test",
    "https://app.test/path",
    "https://app.test?code=secret",
  ]) {
    assert.throws(
      () => setup.requireSetupUiOrigin(origin),
      /current HTTPS UI origin/,
    );
  }
});

test("status requires the actual 200/503 contract, not another successful HTTP status", async () => {
  globalThis.fetch = async () =>
    Response.json(
      {
        stage: "installation",
        state: "awaiting_choice",
        ready: false,
        owners: ["idp"],
      },
      { status: 201 },
    );
  await assert.rejects(setup.fetchSetupStatus("https://node.test/idp"), /201/);
});
