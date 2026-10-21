import assert from "node:assert/strict";
import { createRequire } from "node:module";
import path from "node:path";
const require = createRequire(
    path.resolve(
        import.meta.dirname,
        "../packages/management-client/package.json",
    ),
);
const { DefaultApi, Configuration } = await import(
    require.resolve("@aicacia/management-client/node-join")
);
const basePath = process.argv[2];
await assert.rejects(
    new DefaultApi(new Configuration({ basePath })).pending(),
    (error) => error.response.status === 401,
);
const api = new DefaultApi(
    new Configuration({ basePath, accessToken: process.env.OF_TEST_BEARER }),
);
assert.equal(typeof (await api.acceptance()).accepting, "boolean");
console.log(
    "Built Node-join export qualified against actual host-composed router",
);
