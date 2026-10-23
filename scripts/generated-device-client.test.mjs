import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
    Configuration,
    OwnerCopyCleanupFromJSON,
    OwnerCopyCleanupToJSON,
    SelectedResourcesResponseFromJSON,
    SelectedResourcesResponseToJSON,
    SuperRoutesReplicationApi,
    instanceOfOwnerCopyCleanup,
} from "../packages/device-client/esm/index.js";

const cleanup = {
    cleanupId: "01900000-0000-7000-8000-000000000001",
    deviceId: "01900000-0000-7000-8000-000000000002",
    endpointId: "a".repeat(64),
    oldOwnerSubject: "previous-owner",
    newOwnerSubject: "current-owner",
};
const fields = [
    "cleanupId",
    "deviceId",
    "endpointId",
    "newOwnerSubject",
    "oldOwnerSubject",
];
const options = { credentials: "omit", redirect: "error", cache: "no-store" };

test("current composed Device schema has 50 operations and the exact service cleanup contract", () => {
    const document = JSON.parse(
        readFileSync(new URL("../openapi/device.json", import.meta.url)),
    );
    const operations = Object.values(document.paths).flatMap((item) =>
        Object.values(item).filter((operation) => operation?.operationId),
    );
    assert.equal(operations.length, 50);
    assert.equal(
        new Set(operations.map((operation) => operation.operationId)).size,
        50,
    );
    assert.equal(document.servers[0].url, "https://localhost/device");
    assert.ok(
        Object.keys(document.paths).every(
            (path) =>
                !path.startsWith("/device/") &&
                !path.includes("/internal/"),
        ),
    );
    const operation = document.paths["/replication/owner-cleanup"].post;
    assert.equal(operation.operationId, "owner_cleanup");
    assert.deepEqual(operation.security, [{ authorization: [] }]);
    assert.equal(operation.requestBody.required, true);
    assert.equal(
        operation.requestBody.content["application/json"].schema.$ref,
        "#/components/schemas/OwnerCopyCleanup",
    );
    assert.deepEqual(Object.keys(operation.responses).sort(), [
        "204",
        "401",
        "403",
        "503",
    ]);
    const schema = document.components.schemas.OwnerCopyCleanup;
    assert.deepEqual(Object.keys(schema.properties).sort(), fields);
    assert.deepEqual(schema.required.toSorted(), fields);
    assert.equal(schema.additionalProperties, false);
    const policy = document.components.schemas.SelectedResourcesResponse;
    assert.ok(!policy.required.includes("pendingCleanup"));
    assert.deepEqual(policy.properties.pendingCleanup.oneOf, [
        { type: "null" },
        { $ref: "#/components/schemas/OwnerCopyCleanup" },
    ]);
});

test("built cleanup model preserves every exact binding and requires all five fields", () => {
    assert.deepEqual(OwnerCopyCleanupFromJSON(cleanup), cleanup);
    assert.deepEqual(OwnerCopyCleanupToJSON(cleanup), cleanup);
    assert.equal(instanceOfOwnerCopyCleanup(cleanup), true);
    for (const field of fields) {
        const incomplete = { ...cleanup };
        delete incomplete[field];
        assert.equal(instanceOfOwnerCopyCleanup(incomplete), false, field);
    }
});

test("built selection policy round-trips pending, absent and null cleanup without inventing allowance or resources", () => {
    const policy = {
        deviceId: cleanup.deviceId,
        endpointId: cleanup.endpointId,
        ownerSubject: cleanup.newOwnerSubject,
        adminAllowed: false,
        resources: [],
    };
    for (const wire of [
        policy,
        { ...policy, pendingCleanup: null },
        { ...policy, pendingCleanup: cleanup },
    ]) {
        const decoded = SelectedResourcesResponseFromJSON(wire);
        assert.deepEqual(decoded.pendingCleanup, wire.pendingCleanup);
        assert.equal(decoded.adminAllowed, false);
        assert.deepEqual(decoded.resources, []);
        assert.deepEqual(
            JSON.parse(
                JSON.stringify(SelectedResourcesResponseToJSON(decoded)),
            ),
            wire,
        );
    }
});

test("built cleanup acknowledgement sends one Device prefix, the exact service bearer and tuple, and accepts 204", async () => {
    let captured;
    const api = new SuperRoutesReplicationApi(
        new Configuration({
            basePath: "https://policy.example/device",
            accessToken: "storage-management-admit-token",
            fetchApi: async (url, init) => {
                captured = { url, init };
                return new Response(null, { status: 204 });
            },
        }),
    );
    assert.equal(
        await api.ownerCleanup({ ownerCopyCleanup: cleanup }, options),
        undefined,
    );
    assert.equal(
        captured.url,
        "https://policy.example/device/replication/owner-cleanup",
    );
    assert.equal(captured.init.method, "POST");
    assert.deepEqual(captured.init.headers, {
        "Content-Type": "application/json",
        Authorization: "Bearer storage-management-admit-token",
    });
    assert.deepEqual(JSON.parse(captured.init.body), cleanup);
    for (const [key, value] of Object.entries(options))
        assert.equal(captured.init[key], value);
});

test("built selections response retains service-only cleanup evidence at the owner-relative endpoint", async () => {
    const wire = {
        deviceId: cleanup.deviceId,
        endpointId: cleanup.endpointId,
        ownerSubject: cleanup.newOwnerSubject,
        pendingCleanup: cleanup,
        adminAllowed: false,
        resources: [],
    };
    const api = new SuperRoutesReplicationApi(
        new Configuration({
            basePath: "https://policy.example/device",
            accessToken: "storage-management-read-token",
            fetchApi: async (url, init) => {
                assert.equal(
                    url,
                    `https://policy.example/device/replication/devices/${cleanup.endpointId}/selections`,
                );
                assert.equal(init.method, "GET");
                assert.equal(
                    init.headers.Authorization,
                    "Bearer storage-management-read-token",
                );
                assert.equal(init.body, undefined);
                return Response.json(wire);
            },
        }),
    );
    assert.deepEqual(
        await api.selectedResources(
            { endpointId: cleanup.endpointId },
            options,
        ),
        wire,
    );
});

test("built cleanup acknowledgement preserves denial and unavailability rather than implying completion", async () => {
    for (const status of [401, 403, 503]) {
        const api = new SuperRoutesReplicationApi(
            new Configuration({
                basePath: "https://policy.example/device",
                accessToken: "denied-token",
                fetchApi: async () => new Response(null, { status }),
            }),
        );
        await assert.rejects(
            api.ownerCleanup({ ownerCopyCleanup: cleanup }, options),
            (error) => error.response?.status === status,
        );
    }
});
