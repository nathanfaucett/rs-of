import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
    chmodSync,
    mkdtempSync,
    readFileSync,
    readdirSync,
    rmSync,
    writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
const root = path.resolve(import.meta.dirname, "..");
const realPnpm = spawnSync("which", ["pnpm"], {
    encoding: "utf8",
}).stdout.trim();
function digest() {
    const hash = createHash("sha256");
    function visit(dir) {
        for (const entry of readdirSync(dir, { withFileTypes: true }).sort(
            (a, b) => a.name.localeCompare(b.name),
        )) {
            if (entry.name === "target") continue;
            const name = path.join(dir, entry.name);
            if (entry.isDirectory()) visit(name);
            else hash.update(name).update(readFileSync(name));
        }
    }
    for (const dir of [
        "packages/idp-client/src",
        "packages/management-client/src",
        "packages/storage-client/src",
        "clients/rust",
    ])
        visit(path.join(root, dir));
    return hash.digest("hex");
}
for (const failure of [
    "invalid-schema",
    "generate",
    "biome",
    "tsc",
    "cargo-fmt",
    "cargo-check",
]) {
    test(`${failure} leaves both-language artifacts intact`, {
        timeout: 180000,
    }, () => {
        const temp = mkdtempSync(path.join(tmpdir(), "of-generation-failure-"));
        const before = digest();
        try {
            const executable = path.join(temp, "pnpm");
            writeFileSync(
                executable,
                `#!/usr/bin/env node\nconst {spawnSync} = require('node:child_process');\nconst args = process.argv.slice(2);\nif (args.includes(${JSON.stringify(failure)})) process.exit(37);\nconst result = spawnSync(${JSON.stringify(realPnpm)}, args, {stdio:'inherit'});\nprocess.exit(result.status ?? 1);\n`,
            );
            chmodSync(executable, 0o755);
            const env = { ...process.env, npm_execpath: executable };
            if (failure.startsWith("cargo-")) {
                const cargo = spawnSync("which", ["cargo"], {
                    encoding: "utf8",
                }).stdout.trim();
                const wrapper = path.join(temp, "cargo");
                writeFileSync(
                    wrapper,
                    `#!/usr/bin/env node\nconst {spawnSync} = require('node:child_process');\nconst args = process.argv.slice(2);\nif (args[0] === ${JSON.stringify(failure.slice(6))}) process.exit(38);\nprocess.exit(spawnSync(${JSON.stringify(cargo)}, args, {stdio:'inherit'}).status ?? 1);\n`,
                );
                chmodSync(wrapper, 0o755);
                env.PATH = temp + path.delimiter + env.PATH;
            }
            if (failure === "invalid-schema") {
                const spec = JSON.parse(
                    readFileSync(path.join(root, "openapi/idp.json")),
                );
                spec.paths["/broken"] = {
                    get: {
                        responses: {
                            200: {
                                description: "invalid reference",
                                content: {
                                    "application/json": {
                                        schema: {
                                            $ref: "#/components/schemas/Missing",
                                        },
                                    },
                                },
                            },
                        },
                    },
                };
                env.OPENAPI_SPEC_FILE = path.join(temp, "invalid.json");
                writeFileSync(env.OPENAPI_SPEC_FILE, JSON.stringify(spec));
            }
            const result = spawnSync(
                process.execPath,
                [path.join(root, "scripts/generate-openapi-client.mjs"), "idp"],
                { cwd: root, env, encoding: "utf8", timeout: 170000 },
            );
            assert.notEqual(result.status, 0, result.stdout + result.stderr);
            assert.equal(
                digest(),
                before,
                "failed generation must not alter any prior SDK",
            );
            assert.ok(
                !readdirSync(root).some((name) =>
                    name.startsWith(".openapi-generation-"),
                ),
            );
        } finally {
            rmSync(temp, { recursive: true, force: true });
        }
    });
}

for (const failure of ["check", "fetch"]) {
    test(`cold SDK cache fetch precedes offline checking and preserves artifacts on ${failure} failure`, () => {
        const temp = mkdtempSync(path.join(tmpdir(), "of-generation-cold-"));
        const before = digest();
        try {
            const log = path.join(temp, "cargo.jsonl");
            const pnpm = path.join(temp, "pnpm");
            writeFileSync(
                pnpm,
                `#!/usr/bin/env node
const {cpSync} = require("node:fs");
const path = require("node:path");
const args = process.argv.slice(2);
if (args.includes("generate")) {
    const language = args[args.indexOf("-g") + 1];
    cpSync(path.join(${JSON.stringify(root)}, language === "rust" ? "clients/rust/idp" : "packages/idp-client/src"), args[args.indexOf("-o") + 1], {recursive: true});
}
`,
            );
            const cargo = path.join(temp, "cargo");
            writeFileSync(
                cargo,
                `#!/usr/bin/env node
const {appendFileSync, readFileSync} = require("node:fs");
const path = require("node:path");
const args = process.argv.slice(2);
const manifest = args[args.indexOf("--manifest-path") + 1];
const lock = readFileSync(path.join(path.dirname(manifest), "Cargo.lock"), "utf8");
appendFileSync(${JSON.stringify(log)}, JSON.stringify({args, lock}) + String.fromCharCode(10));
if (args[0] === ${JSON.stringify(failure)}) process.exit(39);
`,
            );
            chmodSync(pnpm, 0o755);
            chmodSync(cargo, 0o755);
            const result = spawnSync(
                process.execPath,
                [path.join(root, "scripts/generate-openapi-client.mjs"), "idp"],
                {
                    cwd: root,
                    env: {
                        ...process.env,
                        npm_execpath: pnpm,
                        PATH: temp + path.delimiter + process.env.PATH,
                    },
                    encoding: "utf8",
                    timeout: 10000,
                },
            );
            assert.match(result.stderr, /cargo failed \(39\)/);
            const calls = readFileSync(log, "utf8")
                .trim()
                .split("\n")
                .map((line) => JSON.parse(line));
            const fetch = calls.findIndex((call) => call.args[0] === "fetch");
            const check = calls.findIndex((call) => call.args[0] === "check");
            assert.ok(
                fetch >= 0,
                "fetch SDK dependencies before offline checking on a cold cache",
            );
            assert.equal(
                calls[fetch].lock,
                readFileSync(
                    path.join(root, "clients/rust/Cargo.lock"),
                    "utf8",
                ),
            );
            assert.ok(calls[fetch].args.includes("--locked"));
            if (failure === "fetch") {
                assert.equal(
                    check,
                    -1,
                    "failed acquisition must stop before checking or publication",
                );
            } else {
                assert.ok(fetch < check);
                assert.deepEqual(calls[fetch].args, [
                    "fetch",
                    "--manifest-path",
                    calls[check].args[2],
                    "--locked",
                ]);
                assert.ok(calls[check].args.includes("--offline"));
                assert.ok(calls[check].args.includes("--locked"));
            }
            assert.equal(digest(), before);
            assert.ok(
                !readdirSync(root).some((name) =>
                    name.startsWith(".openapi-generation-"),
                ),
            );
        } finally {
            rmSync(temp, { recursive: true, force: true });
        }
    });
}
