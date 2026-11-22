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

test("generation reuses canonical setup contracts", () => {
  const root = mkdtempSync(path.join(tmpdir(), "of-canonical-models-"));
  try {
    const output = path.join(root, "client");
    const models = path.join(output, "src/models");
    mkdirSync(models, { recursive: true });
    writeFileSync(
      path.join(models, "mod.rs"),
      "pub mod independent_create_intent;\npub use self::independent_create_intent::IndependentCreateIntent;\npub mod independent_create_request;\npub use self::independent_create_request::IndependentCreateRequest;\n",
    );
    writeFileSync(
      path.join(models, "independent_create_intent.rs"),
      "pub struct IndependentCreateIntent {}\n",
    );
    writeFileSync(
      path.join(models, "independent_create_request.rs"),
      "pub struct IndependentCreateRequest {}\n",
    );
    const shared = path.join(root, "shared");
    extractRustModels(output, "device", shared);
    const index = readFileSync(path.join(shared, "mod.rs"), "utf8");
    assert.match(index, /pub use crate::setup::IndependentCreateIntent;/);
    assert.match(index, /pub use crate::setup::IndependentCreateRequest;/);
    assert.equal(existsSync(path.join(shared, "independent_create_intent.rs")), false);
    assert.equal(existsSync(path.join(shared, "independent_create_request.rs")), false);
    assert.doesNotMatch(
      readFileSync(path.join(models, "mod.rs"), "utf8"),
      /\bindependent_create_intent\b|\bindependent_create_request\b/,
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
