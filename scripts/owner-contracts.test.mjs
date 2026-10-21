import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import test from "node:test";
const root = new URL("../", import.meta.url);
test("exactly three owner contracts and root-workspace Rust clients", () => {
    assert.deepEqual(readdirSync(new URL("openapi/", root)).sort(), [
        "idp.json",
        "management.json",
        "storage.json",
    ]);
    const workspace = readFileSync(new URL("Cargo.toml", root), "utf8");
    for (const owner of ["idp", "management", "storage"]) {
        assert.ok(workspace.includes(`"crates/${owner}-client"`));
        assert.ok(
            existsSync(new URL(`crates/${owner}-client/Cargo.toml`, root)),
        );
    }
    assert.ok(!existsSync(new URL("clients/rust", root)));
});
test("coordinated and independent Management approvals retain distinct semantics", () => {
    const doc = JSON.parse(
        readFileSync(new URL("openapi/management.json", root)),
    );
    assert.ok(doc.paths["/node-joins/{id}/approve"].post.responses["200"]);
    assert.ok(
        doc.paths["/independent/node-joins/{id}/approve"].post.responses["202"],
    );
    assert.notEqual(
        doc.paths["/node-joins/{id}/approve"].post.operationId,
        doc.paths["/independent/node-joins/{id}/approve"].post.operationId,
    );
});
