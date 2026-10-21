import assert from "node:assert/strict";
import { createRequire } from "node:module";
import path from "node:path";
const require = createRequire(
    path.resolve(
        import.meta.dirname,
        "../packages/management-client/package.json",
    ),
);
const { NodeJoinApi: DefaultApi, Configuration } = await import(
    require.resolve("@aicacia/management-client")
);
const basePath = process.argv[2];
await assert.rejects(
    new DefaultApi(
        new Configuration({ basePath: basePath + "/management" }),
    ).nodeJoinPending(),
    (error) => error.response.status === 401,
);
const api = new DefaultApi(
    new Configuration({
        basePath: basePath + "/management",
        accessToken: process.env.OF_TEST_BEARER,
    }),
);
assert.equal(typeof (await api.nodeJoinAcceptance()).accepting, "boolean");
console.log(
    "Built Node-join export qualified against actual host-composed router",
);
