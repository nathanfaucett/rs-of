import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import test from "node:test";

const root = new URL("../", import.meta.url);
test("four owner contracts and service Rust clients, plus the Node setup contract", () => {
  assert.deepEqual(readdirSync(new URL("openapi/", root)).sort(), [
    "device.json",
    "idp.json",
    "management.json",
    "node.json",
    "storage.json",
  ]);
  const workspace = readFileSync(new URL("Cargo.toml", root), "utf8");
  for (const owner of ["idp", "management", "device", "storage"]) {
    const crate = owner === "storage" ? "storage-api-client" : `${owner}-client`;
    assert.ok(workspace.includes(`"services/${crate}"`));
    assert.ok(existsSync(new URL(`services/${crate}/Cargo.toml`, root)));
  }
  assert.ok(!existsSync(new URL("clients/rust", root)));
});
test("Device keeps enrollment and approval without server Join routes", () => {
  for (const owner of ["idp", "device", "management", "storage"]) {
    const doc = JSON.parse(readFileSync(new URL(`openapi/${owner}.json`, root)));
    for (const path of Object.keys(doc.paths))
      assert.doesNotMatch(path, /node-joins|\/setup\/owner\/(join|history)$/);
    if (owner === "device") {
      assert.ok(doc.paths["/devices/enrollments"].post);
      assert.ok(doc.paths["/devices/enrollments/{device_id}/approve"].post);
      assert.ok(!doc.paths["/devices/pairing-accepting"]);
      assert.ok(doc.paths["/devices/pairings"].post);
      assert.ok(doc.paths["/devices/pairings/{device_id}/payload"].get);
      assert.ok(doc.paths["/devices/pairings/{device_id}/approve"].post);
      assert.ok(doc.paths["/devices/{id}"].delete);
    }
  }
});

test("only Device publishes Selection routes", () => {
  const management = JSON.parse(readFileSync(new URL("openapi/management.json", root)));
  assert.ok(management.paths["/applications/{application_id}/roles"]);
  assert.ok(management.paths["/permissions/evaluate"]);
  for (const route of Object.keys(management.paths))
    assert.doesNotMatch(route, /^\/(devices?|replication|resources)(\/|$)/);

  const device = JSON.parse(readFileSync(new URL("openapi/device.json", root)));
  assert.ok(device.paths["/devices/{device_id}/selection"].put);
  assert.ok(!device.paths["/devices/{device_id}/selection"].delete);
  assert.ok(
    device.paths["/devices/{device_id}/selection/{application_id}/{kind}/{resource_id}"].delete
  );
  const storage = JSON.parse(readFileSync(new URL("openapi/storage.json", root)));
  assert.ok(!storage.paths["/sync/connect"]);
});

test("setup contracts expose only owner-local capabilities in every layout", () => {
  for (const owner of ["idp", "device", "storage"]) {
    const doc = JSON.parse(readFileSync(new URL(`openapi/${owner}.json`, root)));
    assert.ok(doc.paths["/setup/owner/status"]);
    for (const route of Object.keys(doc.paths).filter((route) => route.startsWith("/setup/")))
      assert.ok(route.startsWith("/setup/owner/"), `${owner}: retired route ${route}`);
    assert.ok(!doc.components.schemas.CreateResult);
    assert.ok(!doc.components.securitySchemes.setup_authorization);
  }
});
