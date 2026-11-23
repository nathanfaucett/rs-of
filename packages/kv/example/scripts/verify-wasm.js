import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const distAssets = path.join(__dirname, "..", "dist", "assets");
const wasmFiles = fs.readdirSync(distAssets).filter((f) => f.endsWith(".wasm"));

if (wasmFiles.length < 1) {
  console.error("ERROR: Missing WASM file in build output");
  console.error("Expected at least 1 WASM file, found:", wasmFiles.length);
  process.exit(1);
}

console.log("✓ WASM files verified:", wasmFiles.join(", "));
