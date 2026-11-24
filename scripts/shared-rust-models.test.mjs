import assert from "node:assert/strict";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
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
      "pub mod independent_create_intent;\npub use self::independent_create_intent::IndependentCreateIntent;\npub mod independent_create_request;\npub use self::independent_create_request::IndependentCreateRequest;\n"
    );
    writeFileSync(
      path.join(models, "independent_create_intent.rs"),
      "pub struct IndependentCreateIntent {}\n"
    );
    writeFileSync(
      path.join(models, "independent_create_request.rs"),
      "pub struct IndependentCreateRequest {}\n"
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
      /\bindependent_create_intent\b|\bindependent_create_request\b/
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
    writeFileSync(path.join(models, "mod.rs"), "pub mod sample;\npub use self::sample::Sample;\n");
    writeFileSync(
      path.join(models, "sample.rs"),
      `use crate::models;
use serde::{Serialize, Deserialize};
#[derive(Serialize, Deserialize)]
pub struct Sample { pub name: String, pub values: Vec<Box<String>> }
impl std::fmt::Display for Sample {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.name) }
}
`
    );
    const shared = path.join(root, "shared");
    extractRustModels(output, "storage_api", shared);
    const source = readFileSync(path.join(shared, "sample.rs"), "utf8");
    assert.match(source, /#\[cfg\(not\(feature = "std"\)\)\]\nuse alloc::/);
    assert.match(source, /use core::fmt;/);
    assert.match(source, /impl fmt::Display/);
    assert.doesNotMatch(source, /std::/);
    assert.match(
      readFileSync(path.join(models, "mod.rs"), "utf8"),
      /pub use of_model::http::storage_api::\{sample, Sample\};/
    );
    assert.equal(existsSync(path.join(models, "sample.rs")), false);
    assert.match(readFileSync(path.join(shared, "mod.rs"), "utf8"), /pub mod sample;/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("generated models normalize alloc macros, constructors, and enum defaults", () => {
  const root = mkdtempSync(path.join(tmpdir(), "of-normalized-models-"));
  try {
    const output = path.join(root, "client");
    const models = path.join(output, "src/models");
    mkdirSync(models, { recursive: true });
    writeFileSync(
      path.join(models, "mod.rs"),
      "pub mod sample;\npub use self::sample::Sample;\npub mod boxed;\npub use self::boxed::Boxed;\n"
    );
    writeFileSync(
      path.join(models, "sample.rs"),
      `use serde::{Serialize, Deserialize};
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sample { pub name: String, pub values: Vec<Box<String>> }
impl Sample {
    pub fn new(name: String) -> Sample {
        Sample { name, values: Vec::new() }
    }
    pub fn encode(&self) -> String {
        let values = vec![self.name.to_string()];
        format!("{}", values.join(",")).to_owned()
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum State {
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "pending")]
    Pending,
}
impl std::fmt::Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self { Self::Ready => write!(f, "ready"), Self::Pending => write!(f, "pending") }
    }
}
impl Default for State {
    fn default() -> State {
        Self::Ready
    }
}
`
    );
    writeFileSync(
      path.join(models, "boxed.rs"),
      `use serde::{Serialize, Deserialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Boxed { pub value: Box<String> }
impl Boxed {
    pub fn new(value: String) -> Boxed {
        Boxed { value: Box::new(value) }
    }
}
`
    );
    extractRustModels(output, "storage_api", path.join(root, "shared"));
    const source = readFileSync(path.join(root, "shared/sample.rs"), "utf8");
    assert.match(source, /#\[cfg\(not\(feature = "std"\)\)\]\nuse alloc::\{[^\n]*boxed::Box/);
    for (const imported of [
      "format",
      "string::String",
      "string::ToString",
      "borrow::ToOwned",
      "vec",
      "vec::Vec",
    ]) {
      assert.ok(source.includes(imported), imported);
    }
    assert.match(source, /#\[must_use\]\n {4}pub const fn new\(name: String\) -> Self/);
    assert.match(source, /Self \{ name, values: Vec::new\(\) \}/);
    assert.match(source, /PartialEq, Eq, Serialize, Deserialize/);
    assert.match(source, /Serialize, Deserialize, Default\)/);
    assert.match(source, /#\[default\]\n {4}Ready,/);
    assert.doesNotMatch(source, /impl Default for State/);
    assert.match(source, /use core::fmt;/);
    const boxed = readFileSync(path.join(root, "shared/boxed.rs"), "utf8");
    assert.match(boxed, /#\[must_use\]\n {4}pub fn new\(value: String\) -> Self/);
    assert.doesNotMatch(boxed, /pub const fn new/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("generated Eq derives do not cross floating-point model references", () => {
  const root = mkdtempSync(path.join(tmpdir(), "of-eq-models-"));
  try {
    const output = path.join(root, "client");
    const models = path.join(output, "src/models");
    mkdirSync(models, { recursive: true });
    writeFileSync(
      path.join(models, "mod.rs"),
      "pub mod float;\npub use self::float::Float;\npub mod wrapper;\npub use self::wrapper::Wrapper;\npub mod text;\npub use self::text::Text;\n"
    );
    const declaration = "#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]\n";
    writeFileSync(
      path.join(models, "float.rs"),
      `use serde::{Serialize, Deserialize};\n${declaration}pub struct Float { pub value: f64 }\n`
    );
    writeFileSync(
      path.join(models, "wrapper.rs"),
      `use crate::models;\nuse serde::{Serialize, Deserialize};\n${declaration}pub struct Wrapper { pub value: models::Float }\n`
    );
    writeFileSync(
      path.join(models, "text.rs"),
      `use serde::{Serialize, Deserialize};\n${declaration}pub struct Text { pub value: String }\n`
    );
    extractRustModels(output, "storage_api", path.join(root, "shared"));
    for (const file of ["float.rs", "wrapper.rs"]) {
      assert.doesNotMatch(readFileSync(path.join(root, "shared", file), "utf8"), /PartialEq, Eq/);
    }
    assert.match(readFileSync(path.join(root, "shared/text.rs"), "utf8"), /PartialEq, Eq/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
