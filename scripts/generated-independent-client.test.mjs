import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
    Configuration,
    DefaultApi,
    IndependentCreateIntentToJSON,
    IndependentCreateRequestToJSON,
} from "../packages/idp-client/esm/independent/index.js";

const operation = {
    purpose: "join",
    operationNonce: "1".repeat(32),
    administrator: "",
    node: "Joining Node",
    idp: "https://target.test/idp",
    management: "https://target.test/management",
    storage: "https://target.test/storage",
    issuer: "",
    peer: "https://source.test/management#" + "d".repeat(64),
};
const options = { credentials: "omit", redirect: "error", cache: "no-store" };

test("complete independent schema includes worker handoffs with owner-relative bases", () => {
    const document = JSON.parse(
        readFileSync(
            new URL(
                "../openapi/idp-independent.json",
                import.meta.url,
            ),
        ),
    );
    assert.deepEqual(Object.keys(document.paths).sort(), [
        "/setup/admission",
        "/setup/cancel",
        "/setup/create",
        "/setup/independent/challenge",
        "/setup/independent/history",
        "/setup/independent/public",
        "/setup/independent/register",
        "/setup/intent",
        "/setup/join",
        "/setup/status",
    ]);
    assert.ok(
        JSON.stringify(document).includes("/setup/independent/challenge"),
    );
    assert.ok(
        JSON.stringify(document).includes("/setup/independent/register"),
    );
    assert.ok(JSON.stringify(document).includes("/setup/independent/history"));
});
test("generated Join sends empty issuer/admin without password to each exact owner base", async () => {
    for (const owner of ["idp", "management", "storage"]) {
        const api = new DefaultApi(
            new Configuration({
                basePath: operation[owner],
                apiKey: "Setup " + owner,
                fetchApi: async (url, init) => {
                    assert.equal(url, operation[owner] + "/setup/join");
                    assert.equal(init.headers.Authorization, "Setup " + owner);
                    const { operationNonce, ...fields } = operation;
                    assert.deepEqual(JSON.parse(init.body), {
                        ...fields,
                        operation_nonce: operationNonce,
                    });
                    return new Response(null, { status: 202 });
                },
            }),
        );
        const raw = await api.joinRaw(
            { independentCreateRequest: operation },
            options,
        );
        assert.equal(raw.raw.status, 202);
    }
});
test("generated serializers preserve the wire tuple, nullable peer and optional IdP-only password", () => {
    const create = {
        ...operation,
        purpose: "create",
        administrator: "Alice",
        issuer: operation.idp,
        peer: null,
    };
    const intent = JSON.parse(
        JSON.stringify(IndependentCreateIntentToJSON(create)),
    );
    assert.equal(intent.operation_nonce, operation.operationNonce);
    assert.equal(intent.peer, null);
    assert.ok(!("password" in intent));
    assert.ok(!("operationNonce" in intent));
    assert.equal(
        JSON.parse(
            JSON.stringify(
                IndependentCreateRequestToJSON({
                    ...create,
                    password: "protected-password",
                }),
            ),
        ).password,
        "protected-password",
    );
    assert.ok(
        !(
            "password" in
            JSON.parse(JSON.stringify(IndependentCreateRequestToJSON(create)))
        ),
    );
});
test("public observation carries no setup code and recovery/cancel retain distinct status semantics", async () => {
    const calls = [];
    const api = new DefaultApi(
        new Configuration({
            basePath: operation.management,
            apiKey: "Setup code",
            fetchApi: async (url, init) => {
                calls.push({ url, init });
                if (url.endsWith("/status"))
                    return Response.json({
                        stage: "installation",
                        state: "joining",
                        ready: false,
                        owners: ["management"],
                    });
                if (url.endsWith("/public"))
                    return Response.json({
                        owner: "management",
                        operation: IndependentCreateIntentToJSON({
                            ...operation,
                            issuer: "https://installation.test/idp",
                        }),
                        clients: [],
                        facts: {},
                        registered: [],
                    });
                return new Response(null, { status: 204 });
            },
        }),
    );
    await api.status(options);
    await api.ownerPublic(options);
    for (const { init } of calls)
        assert.equal(init.headers.Authorization, undefined);
    assert.equal((await api.recoverRaw(options)).raw.status, 204);
    assert.equal(calls.at(-1).init.body, undefined);
    assert.equal(calls.at(-1).init.headers.Authorization, "Setup code");
    assert.equal(
        (
            await api.cancelRaw(
                {
                    independentCreateIntent: {
                        ...operation,
                        issuer: "https://installation.test/idp",
                    },
                },
                options,
            )
        ).raw.status,
        204,
    );
    assert.ok(!("password" in JSON.parse(calls.at(-1).init.body)));
});
