import { mkdirSync, readFileSync, readdirSync, renameSync, writeFileSync } from "node:fs";
import path from "node:path";

export function extractRustModels(output, owner, shared) {
  const models = path.join(output, "src/models");
  const index = readFileSync(path.join(models, "mod.rs"), "utf8");
  const names = [
    ...index.matchAll(/pub mod (\w+);/g),
    ...index.matchAll(/pub use self::\w+::(\w+);/g),
  ].map((match) => match[1]);
  if (!names.length) throw new Error("Generated Rust models are missing");
  for (const file of readdirSync(models).filter(
    (file) => file.endsWith(".rs") && file !== "mod.rs"
  )) {
    const target = path.join(models, file);
    let source = readFileSync(target, "utf8").replaceAll("std::fmt", "core::fmt");
    const imports = [];
    for (const [symbol, imported] of [
      ["Box", "boxed::Box"],
      ["String", "string::String"],
      ["Vec", "vec::Vec"],
    ]) {
      if (new RegExp(`\\b${symbol}\\b`).test(source)) imports.push(imported);
    }
    if (source.includes(".to_string()")) imports.push("string::ToString");
    source = source.replace(
      "use crate::models;",
      source.includes("models::") ? `use crate::http::${owner} as models;` : ""
    );
    source = source.replaceAll("crate::models::", `crate::http::${owner}::`);
    if (source.includes("std::")) throw new Error(`Generated model requires std: ${file}`);
    writeFileSync(target, (imports.length ? `use alloc::{${imports.join(", ")}};\n` : "") + source);
  }
  mkdirSync(path.dirname(shared), { recursive: true });
  renameSync(models, shared);
  mkdirSync(models);
  writeFileSync(
    path.join(models, "mod.rs"),
    `pub use of_model::http::${owner}::{${names.join(", ")}};\n`
  );
}
