import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import test from "node:test";
const root = new URL("../", import.meta.url);
test("exactly four owner contracts and service Rust clients", () => {
    assert.deepEqual(readdirSync(new URL("openapi/", root)).sort(), [
        "device.json",
        "idp.json",
        "management.json",
        "storage.json",
    ]);
    const workspace = readFileSync(new URL("Cargo.toml", root), "utf8");
    for (const owner of ["idp", "management", "device", "storage"]) {
        const crate = owner === "storage" ? "storage-api-client" : owner + "-client";
        assert.ok(workspace.includes("\"services/" + crate + "\""));
        assert.ok(existsSync(new URL("services/" + crate + "/Cargo.toml", root)));
    }
    assert.ok(!existsSync(new URL("clients/rust", root)));
});
test("coordinated and independent Device approvals retain distinct semantics", () => {
    const doc = JSON.parse(readFileSync(new URL("openapi/device.json", root)));
    assert.ok(doc.paths["/node-joins/{id}/approve"].post.responses["200"]);
    assert.ok(
        doc.paths["/independent/node-joins/{id}/approve"].post.responses["202"],
    );
    assert.notEqual(
        doc.paths["/node-joins/{id}/approve"].post.operationId,
        doc.paths["/independent/node-joins/{id}/approve"].post.operationId,
    );
});

test("Management publishes policy administration and Selection without Device authority", () => {
    const doc = JSON.parse(
        readFileSync(new URL("openapi/management.json", root)),
    );
    const selection = "/devices/{device_id}/sync-selections";
    assert.ok(doc.paths["/applications/{application_id}/roles"]);
    assert.ok(doc.paths["/permissions/evaluate"]);
    assert.ok(doc.paths[selection]);
    for (const route of Object.keys(doc.paths))
        assert.ok(
            route === selection || !/^\/(devices?|replication|node-joins)(\/|$)/.test(route),
            route,
        );
});

test("setup contracts expose only owner-local capabilities in every layout", () => {
    for (const owner of ["idp", "device", "storage"]) {
        const doc = JSON.parse(
            readFileSync(new URL(`openapi/${owner}.json`, root)),
        );
        assert.ok(doc.paths["/setup/owner/status"]);
        for (const route of Object.keys(doc.paths).filter((route) =>
            route.startsWith("/setup/"),
        ))
            assert.ok(
                route.startsWith("/setup/owner/"),
                `${owner}: retired route ${route}`,
            );
        assert.ok(!doc.components.schemas.CreateResult);
        assert.ok(!doc.components.securitySchemes.setup_authorization);
    }
});
