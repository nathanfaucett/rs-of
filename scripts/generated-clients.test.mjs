import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { createRequire, registerHooks } from "node:module";
import path from "node:path";
import test from "node:test";

const root = path.resolve(import.meta.dirname, "../packages");
const { transpileModule, ModuleKind, ScriptTarget } = createRequire(
  path.join(root, "idp-client/package.json"),
)("typescript");

// Execute generated TypeScript in memory; never build or publish package outputs.
registerHooks({
  resolve(specifier, context, nextResolve) {
    if (
      specifier.endsWith(".js") &&
      context.parentURL?.includes("-client/src/")
    ) {
      const candidate = new URL(
        specifier.replace(/\.js$/, ".ts"),
        context.parentURL,
      );
      if (existsSync(candidate))
        return { url: candidate.href, shortCircuit: true };
    }
    return nextResolve(specifier, context);
  },
  load(url, context, nextLoad) {
    if (url.endsWith(".ts") && url.includes("-client/src/")) {
      return {
        format: "module",
        source: transpileModule(readFileSync(new URL(url), "utf8"), {
          compilerOptions: {
            module: ModuleKind.ESNext,
            target: ScriptTarget.ES2021,
          },
        }).outputText,
        shortCircuit: true,
      };
    }
    return nextLoad(url, context);
  },
});
const source = (service) =>
  path.join(
    root,
    `${service}-client/src`,
    ...(service === "storage" ? ["generated"] : []),
  );

test("Management owns Device clients and models while IdP retains signer and OAuth contracts", () => {
  const directory = source("idp");
  const api = readFileSync(path.join(directory, "apis/DefaultApi.ts"), "utf8");
  const management = readFileSync(
    path.join(source("management"), "apis/DefaultApi.ts"),
    "utf8",
  );
  for (const operation of [
    "device",
    "enrollDevice",
    "listDevices",
    "lookupDeviceIdentity",
    "pairingAcceptance",
    "revokeDevice",
    "revokeSelf",
    "setPairingAcceptance",
    "transferDevice",
    "trustedDevices",
    "updateDevice",
  ]) {
    assert.match(management, new RegExp(`async ${operation}\\s*\\(`));
    assert.doesNotMatch(api, new RegExp(`async ${operation}\\s*\\(`));
  }
  assert.ok(!existsSync(path.join(directory, "apis/SuperRoutesDevicesApi.ts")));
  for (const model of [
    "DeviceInfo",
    "DeviceEnrollment",
    "DeviceState",
    "DeviceTransferRequest",
    "UpdateDeviceRequest",
  ]) {
    assert.ok(
      existsSync(path.join(source("management"), `models/${model}.ts`)),
    );
    assert.ok(!existsSync(path.join(directory, `models/${model}.ts`)));
  }
  assert.match(api, /async deviceAuth\s*\(/);
  assert.match(api, /async deviceVerify\s*\(/);
  assert.match(
    readFileSync(
      path.join(directory, "apis/SuperRoutesReplicaSignersApi.ts"),
      "utf8",
    ),
    /async enrollReplicaSigner\s*\(/,
  );
  assert.ok(!existsSync(path.join(directory, "apis/SuperRoutesSetupApi.ts")));
  assert.ok(
    !readdirSync(path.join(directory, "models")).some((name) =>
      name.startsWith("SetupBootstrap"),
    ),
  );
  assert.doesNotMatch(
    readFileSync(path.join(directory, "apis/index.ts"), "utf8"),
    /SuperRoutesSetupApi/,
  );
});

test("Management selected resources requires and round-trips the current device policy", async () => {
  const filename = path.join(
    source("management"),
    "models/SelectedResourcesResponse.ts",
  );
  const model = readFileSync(filename, "utf8");
  for (const [field, type] of [
    ["deviceId", "string"],
    ["endpointId", "string"],
    ["ownerSubject", "string"],
    ["adminAllowed", "boolean"],
    ["resources", "Array<SelectedResource>"],
  ]) {
    assert.ok(model.includes(`${field}: ${type};`), `${field} is required`);
    assert.ok(!model.includes(`${field}?:`), `${field} is not optional`);
  }
  const {
    SelectedResourcesResponseFromJSON,
    SelectedResourcesResponseToJSON,
    instanceOfSelectedResourcesResponse,
  } = await import(filename);
  const policy = {
    deviceId: "device-one",
    endpointId: "endpoint-one",
    ownerSubject: "owner-one",
    adminAllowed: false,
    resources: [
      {
        ownerSubject: "owner-one",
        applicationId: "application-one",
        kind: "database",
        resourceId: "resource-one",
      },
    ],
  };
  assert.deepEqual(SelectedResourcesResponseFromJSON(policy), policy);
  assert.deepEqual(SelectedResourcesResponseToJSON(policy), policy);
  assert.equal(instanceOfSelectedResourcesResponse(policy), true);
  for (const field of Object.keys(policy)) {
    const incomplete = { ...policy };
    delete incomplete[field];
    assert.equal(instanceOfSelectedResourcesResponse(incomplete), false, field);
  }
});

for (const service of ["idp", "management", "storage"]) {
  test(`${service} generated transport prefix occurs only in its server base`, () => {
    const directory = source(service);
    assert.match(
      readFileSync(path.join(directory, "runtime.ts"), "utf8"),
      new RegExp(`BASE_PATH = "https://${service}\\.example/${service}"`),
    );
    for (const file of readdirSync(path.join(directory, "apis")).filter(
      (name) => name.endsWith(".ts"),
    )) {
      const api = readFileSync(path.join(directory, "apis", file), "utf8");
      assert.doesNotMatch(
        api,
        new RegExp(`(?:path:|urlPath =) [\x27\x22\x60]/${service}/`),
      );
      assert.doesNotMatch(api, /x-internal-service|\/internal\//);
    }
  });
  test(`${service} client sends one transport prefix and only the configured bearer`, async () => {
    const { Configuration } = await import(
      path.join(source(service), "runtime.ts")
    );
    const { DefaultApi } = await import(
      path.join(source(service), "apis/DefaultApi.ts")
    );
    let captured;
    const configuration = new Configuration({
      basePath: `https://node.example/${service}`,
      accessToken: "test-bearer",
      fetchApi: async (url, init) => {
        captured = { url, init };
        return new Response("{}", {
          status: 200,
          headers: { "Content-Type": "application/json" },
        });
      },
    });
    const api = new DefaultApi(configuration);
    await api.healthRaw();
    assert.equal(captured.url, `https://node.example/${service}/health`);
    assert.ok(
      !Object.keys(captured.init.headers).some((name) =>
        name.toLowerCase().startsWith("x-internal"),
      ),
    );
    const opts =
      service === "idp"
        ? await api.getUserRequestOpts({ userId: "user/one" })
        : service === "management"
          ? await api.evaluatePermissionRequestOpts({
              permissionEvaluationRequest: {
                requestId: "exact-request",
                subject: { type: "user", id: "actual-user" },
                action: "devices_transfer",
                target: {
                  scope: "installation",
                  resource: {
                    type: "device",
                    id: "device-one",
                  },
                },
              },
            })
          : await new (
              await import(path.join(source(service), "apis/ResourcesApi.ts"))
            ).ResourcesApi(configuration).listFileSystemsRequestOpts();
    assert.equal(opts.headers.Authorization, "Bearer test-bearer");
    if (service === "idp") {
      assert.equal(opts.path, "/users/user%2Fone");
    }
    if (service === "management") {
      assert.equal(opts.path, "/permissions/evaluate");
      assert.deepEqual(opts.body, {
        requestId: "exact-request",
        subject: { type: "user", id: "actual-user" },
        action: "devices_transfer",
        target: {
          scope: "installation",
          resource: { type: "device", id: "device-one" },
        },
      });
    }
  });
}


test("Management transfer uses its owner prefix, escaped Device ID and exact bearer", async () => {
  const { Configuration } = await import(path.join(source("management"), "runtime.ts"));
  const { DefaultApi } = await import(path.join(source("management"), "apis/DefaultApi.ts"));
  let captured;
  const api = new DefaultApi(new Configuration({
    basePath: "https://node.example/management",
    accessToken: "management-user-token",
    fetchApi: async (url, init) => {
      captured = { url, init };
      return new Response(null, { status: 204 });
    },
  }));
  await api.transferDevice({ deviceId: "device/one", deviceTransferRequest: { ownerSubject: "next-owner" } });
  assert.equal(captured.url, "https://node.example/management/devices/device%2Fone/owner");
  assert.equal(captured.init.method, "PUT");
  assert.equal(captured.init.headers.Authorization, "Bearer management-user-token");
  assert.deepEqual(JSON.parse(captured.init.body), { ownerSubject: "next-owner" });
});

test("IdP active User lookup is separate from Management Device authority", async () => {
  const { Configuration } = await import(path.join(source("idp"), "runtime.ts"));
  const { SuperRoutesActiveUserApi } = await import(path.join(source("idp"), "apis/SuperRoutesActiveUserApi.ts"));
  let captured;
  const api = new SuperRoutesActiveUserApi(new Configuration({
    basePath: "https://identity.example/idp",
    accessToken: "idp-user-lookup-service-token",
    fetchApi: async (url, init) => {
      captured = { url, init };
      return new Response(null, { status: 204 });
    },
  }));
  await api.activeUser({ userId: "user/one" });
  assert.equal(captured.url, "https://identity.example/idp/users/user%2Fone/active");
  assert.equal(captured.init.method, "GET");
  assert.equal(captured.init.headers.Authorization, "Bearer idp-user-lookup-service-token");
});
