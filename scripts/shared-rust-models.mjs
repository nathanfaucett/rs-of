import {
  mkdirSync,
  readdirSync,
  readFileSync,
  renameSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";

const canonicalModels = {
  challenge: ["setup", "Challenge"],
  independent_create_intent: ["setup", "IndependentCreateIntent"],
  independent_create_request: ["setup", "IndependentCreateRequest"],
  independent_owner_public: ["setup", "IndependentOwnerPublic"],
  create_user_request: ["oauth_user", "CreateUserRequest"],
  update_user_info_request: ["oauth_user", "UpdateUserInfoRequest"],
};

function normalizeRustModel(source, owner, deriveEq) {
  source = source.replaceAll("std::fmt", "core::fmt");
  source = source.replace(
    /#\[derive\((?<traits>[^\]]+)\)\]\npub enum (?<name>\w+) \{\n(?<body>[\s\S]*?)\n\}(?<between>[\s\S]*?)\nimpl Default for \k<name> \{\n {4}fn default\(\) -> (?:\k<name>|Self) \{\n {8}Self::(?<variant>\w+)\n {4}\}\n\}/g,
    (match, ...args) => {
      const { traits, body, between, variant } = args.at(-1);
      if (/^ {4}\w+\(/m.test(body)) return match;
      const derived = traits.split(", ").includes("Default") ? traits : `${traits}, Default`;
      const marked = body.replace(
        new RegExp(`    ${variant}(,?)`),
        `    #[default]\n    ${variant}$1`
      );
      return `#[derive(${derived})]\npub enum ${args.at(-1).name} {\n${marked}\n}${between}`;
    }
  );
  source = source.replace(/impl (?<name>\w+) \{\n(?<body>[\s\S]*?)\n\}/g, (block, ...args) => {
    let { name, body } = args.at(-1);
    if (!body.includes("pub fn new(")) return block;
    body = body
      .replace(new RegExp(`\\) -> ${name} \\{`), ") -> Self {")
      .replace(new RegExp(`^(\\s*)${name} \\{`, "gm"), "$1Self {");
    const constFn = body.includes("Box::new") ? "" : "const ";
    const mustUse = body.includes("#[must_use]") ? "" : "#[must_use]\n    ";
    body = body.replace("    pub fn new(", `    ${mustUse}pub ${constFn}fn new(`);
    return `impl ${name} {\n${body}\n}`;
  });
  source = source.replace(/Self::(\w+)\(Default::default\(\)\)/g, (expression, variant) => {
    const inner = source.match(new RegExp(`\\b${variant}\\((String|Box<[^>]+>)\\)`));
    if (!inner) return expression;
    return `Self::${variant}(${inner[1] === "String" ? "String" : "Box"}::default())`;
  });
  source = source.replace(/^\/{3}\s*\n/gm, "");
  if (deriveEq)
    source = source.replaceAll(
      "PartialEq, Serialize, Deserialize",
      "PartialEq, Eq, Serialize, Deserialize"
    );
  const usesFmt = source.includes("core::fmt::");
  source = source.replaceAll("core::fmt::", "fmt::");
  const imports = [];
  for (const [pattern, imported] of [
    [/\bBox\b/, "boxed::Box"],
    [/\bString\b/, "string::String"],
    [/\bVec\b/, "vec::Vec"],
    [/\.to_string\(\)/, "string::ToString"],
    [/\.to_owned\(\)/, "borrow::ToOwned"],
    [/(?<![\w:])format!/, "format"],
    [/(?<![\w:])vec!/, "vec"],
  ]) {
    if (pattern.test(source)) imports.push(imported);
  }
  if (source.includes("std::")) throw new Error("Generated model requires std");
  source = source.replace(
    "use crate::models;",
    source.includes("models::") ? `use crate::http::${owner} as models;` : ""
  );
  source = source.replaceAll("crate::models::", `crate::http::${owner}::`);
  return (
    (usesFmt ? "use core::fmt;\n" : "") +
    (imports.length ? `#[cfg(not(feature = "std"))]\nuse alloc::{${imports.join(", ")}};\n` : "") +
    source
  );
}

export function extractRustModels(output, owner, shared) {
  const models = path.join(output, "src/models");
  let index = readFileSync(path.join(models, "mod.rs"), "utf8");
  const names = [
    ...index.matchAll(/pub mod (\w+);/g),
    ...index.matchAll(/pub use self::\w+::(\w+);/g),
  ]
    .map((match) => match[1])
    .filter((name) => !(name in canonicalModels));
  if (!names.length) throw new Error("Generated Rust models are missing");
  const modelNames = new Map(
    [...index.matchAll(/pub use self::(\w+)::(\w+);/g)].map((match) => [match[1], match[2]])
  );
  const files = readdirSync(models).filter((file) => file.endsWith(".rs") && file !== "mod.rs");
  const sources = new Map(
    files
      .filter((file) => !canonicalModels[path.basename(file, ".rs")])
      .map((file) => [file, readFileSync(path.join(models, file), "utf8")])
  );
  const nonEq = new Set(
    [...sources]
      .filter(([, source]) => /\bf(?:32|64)\b/.test(source))
      .map(([file]) => modelNames.get(path.basename(file, ".rs")))
  );
  let changed = true;
  while (changed) {
    changed = false;
    for (const [file, source] of sources) {
      const name = modelNames.get(path.basename(file, ".rs"));
      if (
        !nonEq.has(name) &&
        [...nonEq].some((referenced) => source.includes(`models::${referenced}`))
      ) {
        nonEq.add(name);
        changed = true;
      }
    }
  }
  for (const file of files) {
    const target = path.join(models, file);
    const module = path.basename(file, ".rs");
    const canonical = canonicalModels[module];
    if (canonical) {
      const [namespace, name] = canonical;
      index = index.replace(
        `pub mod ${module};\npub use self::${module}::${name};`,
        `pub use crate::${namespace}::${name};`
      );
      unlinkSync(target);
      continue;
    }
    const source = sources.get(file);
    try {
      writeFileSync(target, normalizeRustModel(source, owner, !nonEq.has(modelNames.get(module))));
    } catch {
      throw new Error(`Generated model requires std: ${file}`);
    }
  }
  writeFileSync(path.join(models, "mod.rs"), index);
  mkdirSync(path.dirname(shared), { recursive: true });
  renameSync(models, shared);
  mkdirSync(models);
  writeFileSync(
    path.join(models, "mod.rs"),
    `pub use of_model::http::${owner}::{${names.join(", ")}};\n`
  );
}
