import {
  mkdirSync,
  readFileSync,
  readdirSync,
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
  independent_peer: ["setup", "IndependentPeer"],
  independent_join_request: ["setup", "IndependentJoinRequest"],
  independent_join_approval: ["setup", "IndependentJoinApproval"],
  node_join_request: ["node_join", "NodeJoinRequest"],
  node_join_approval: ["node_join", "NodeJoinApproval"],
  create_user_request: ["oauth_user", "CreateUserRequest"],
  update_user_info_request: ["oauth_user", "UpdateUserInfoRequest"],
};

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
  for (const file of readdirSync(models).filter(
    (file) => file.endsWith(".rs") && file !== "mod.rs",
  )) {
    const target = path.join(models, file);
    const module = path.basename(file, ".rs");
    const canonical = canonicalModels[module];
    if (canonical) {
      const [namespace, name] = canonical;
      index = index.replace(
        `pub mod ${module};\npub use self::${module}::${name};`,
        `pub use crate::${namespace}::${name};`,
      );
      unlinkSync(target);
      continue;
    }
    let source = readFileSync(target, "utf8").replaceAll(
      "std::fmt",
      "core::fmt",
    );
    if (module === "sync_session_response") {
      source = source.replace(
        /\/\/\/\n#\[derive[\s\S]*$/,
        "pub use crate::storage::sync::SyncStatus as Status;\n",
      );
    }
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
      source.includes("models::") ? `use crate::http::${owner} as models;` : "",
    );
    source = source.replaceAll("crate::models::", `crate::http::${owner}::`);
    if (source.includes("std::"))
      throw new Error(`Generated model requires std: ${file}`);
    writeFileSync(
      target,
      (imports.length ? `use alloc::{${imports.join(", ")}};\n` : "") + source,
    );
  }
  writeFileSync(path.join(models, "mod.rs"), index);
  mkdirSync(path.dirname(shared), { recursive: true });
  renameSync(models, shared);
  mkdirSync(models);
  writeFileSync(
    path.join(models, "mod.rs"),
    `pub use of_model::http::${owner}::{${names.join(", ")}};\n`,
  );
}
