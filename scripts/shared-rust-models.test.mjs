import assert from "node:assert/strict";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { extractRustModels } from "./shared-rust-models.mjs";

test("generation reuses canonical setup and join contracts", () => {
  const root = mkdtempSync(path.join(tmpdir(), "of-canonical-models-"));
  try {
    const output = path.join(root, "client");
    const models = path.join(output, "src/models");
    mkdirSync(models, { recursive: true });
    writeFileSync(
      path.join(models, "mod.rs"),
      "pub mod independent_peer;\npub use self::independent_peer::IndependentPeer;\npub mod node_join_request;\npub use self::node_join_request::NodeJoinRequest;\n",
    );
    writeFileSync(
      path.join(models, "independent_peer.rs"),
      "pub struct IndependentPeer {}\n",
    );
    writeFileSync(
      path.join(models, "node_join_request.rs"),
      "pub struct NodeJoinRequest {}\n",
    );
    const shared = path.join(root, "shared");
    extractRustModels(output, "device", shared);
    const index = readFileSync(path.join(shared, "mod.rs"), "utf8");
    assert.match(index, /pub use crate::setup::IndependentPeer;/);
    assert.match(index, /pub use crate::node_join::NodeJoinRequest;/);
    assert.equal(existsSync(path.join(shared, "independent_peer.rs")), false);
    assert.equal(existsSync(path.join(shared, "node_join_request.rs")), false);
    assert.doesNotMatch(
      readFileSync(path.join(models, "mod.rs"), "utf8"),
      /\bindependent_peer\b|\bnode_join_request\b/,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("generated clients use alloc-only models from of-model", () => {
  const root = mkdtempSync(path.join(tmpdir(), "of-shared-models-"));
  try {
    const output = path.join(root, "client");
    const models = path.join(output, "src/models");
    mkdirSync(models, { recursive: true });
    writeFileSync(
      path.join(models, "mod.rs"),
      "pub mod sample;\npub use self::sample::Sample;\n",
    );
    writeFileSync(
      path.join(models, "sample.rs"),
      `use crate::models;
use serde::{Serialize, Deserialize};
#[derive(Serialize, Deserialize)]
pub struct Sample { pub name: String, pub values: Vec<Box<String>> }
impl std::fmt::Display for Sample {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.name) }
}
`,
    );
    const shared = path.join(root, "shared");
    extractRustModels(output, "storage_api", shared);
    const source = readFileSync(path.join(shared, "sample.rs"), "utf8");
    assert.match(source, /use alloc::/);
    assert.match(source, /core::fmt::Display/);
    assert.doesNotMatch(source, /std::/);
    assert.match(
      readFileSync(path.join(models, "mod.rs"), "utf8"),
      /pub use of_model::http::storage_api::\{sample, Sample\};/,
    );
    assert.equal(existsSync(path.join(models, "sample.rs")), false);
    assert.match(
      readFileSync(path.join(shared, "mod.rs"), "utf8"),
      /pub mod sample;/,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
