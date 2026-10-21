import assert from "node:assert/strict";
import { createRequire } from "node:module";
import path from "node:path";
const root = path.resolve(import.meta.dirname, "..");
const idp = await import(
    createRequire(path.join(root, "packages/idp-client/package.json")).resolve(
        "@aicacia/idp-client",
    )
);
const storage = await import(
    createRequire(
        path.join(root, "packages/storage-client/package.json"),
    ).resolve("@aicacia/storage-client/generated")
);
const [basePath, storageBase, independentBase] = process.argv.slice(2);
const api = new idp.DefaultApi(new idp.Configuration({ basePath }));
const result = await api.token({
    grantType: "password",
    clientId: "human",
    username: "administrator",
    password: "valid-password",
    scope: "openid",
    authorization: "Basic aHVtYW46aHVtYW4tc2VjcmV0",
});
assert.ok(result.accessToken);
await assert.rejects(
    api.token({
        grantType: "client_credentials",
        clientId: "missing",
        clientSecret: "invalid",
    }),
    (error) => error.response.status >= 400,
);
const catalog = new storage.ResourcesApi(
    new storage.Configuration({ basePath: storageBase }),
);
await assert.rejects(catalog.listFileSystems(), (error) =>
    [401, 403].includes(error.response.status),
);
const { StorageClient } = await import(
    createRequire(
        path.join(root, "packages/storage-client/package.json"),
    ).resolve("@aicacia/storage-client")
);
const consumer = new StorageClient({
    idpBaseUrl: basePath,
    storageBaseUrl: storageBase,
    clientId: "human",
    bearerToken: () => result.accessToken,
    fetch: (input, init) =>
        fetch(input, {
            ...init,
            headers: {
                ...init?.headers,
                ...(new URL(input).origin === new URL(basePath).origin
                    ? { Authorization: "Basic aHVtYW46aHVtYW4tc2VjcmV0" }
                    : {}),
            },
        }),
});
assert.ok((await consumer.listFileSystems()).length);
const filesystem = await consumer.createFileSystem("Generated consumer");
assert.equal(filesystem.name, "Generated consumer");
const socket = await consumer.openSocket(filesystem.id);
socket.close();
for (const owner of ["idp", "management", "storage"]) {
    const require = createRequire(
        path.join(root, `packages/${owner}-client/package.json`),
    );
    const sdk = await import(
        require.resolve(
            `@aicacia/${owner}-client${owner === "storage" ? "/generated" : ""}`,
        )
    );
    assert.ok(sdk.IndependentSetupApi);
    assert.ok(sdk.IndependentJoinApi);
    assert.ok(sdk.OwnerSetupApi);
}
console.log(
    "Built package exports and generated operations qualified against actual owner routers",
);

const owner = await import(
    createRequire(path.join(root, "packages/idp-client/package.json")).resolve(
        "@aicacia/idp-client",
    )
);
const setup = new owner.IndependentSetupApi(
    new owner.Configuration({ basePath: independentBase }),
);
assert.equal((await setup.independentSetupStatus()).ready, false);
await assert.rejects(
    setup.independentSetupRecover(),
    (error) => error.response.status === 401,
);
await assert.rejects(
    setup.independentSetupChallenge({
        challenge: {
            challenge: "a".repeat(64),
            recipient: "management",
            registration: { client_id: "unregistered" },
            operation: {
                administrator: "",
                idp: "",
                issuer: "",
                management: "",
                storage: "",
                node: "",
                operationNonce: "",
                purpose: "join",
                peer: null,
            },
        },
    }),
    (error) => error.response.status === 403,
);
