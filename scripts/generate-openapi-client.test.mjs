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
const realPnpm = spawnSync("which", ["pnpm"], { encoding: "utf8" }).stdout.trim();
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
