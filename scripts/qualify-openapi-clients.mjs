import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
const root = path.resolve(import.meta.dirname, "..");
const pnpm = process.env.npm_execpath || "pnpm";
function run(command, args) {
    const result = spawnSync(command, args, {
        cwd: root,
        stdio: "inherit",
        timeout: 600000,
    });
    if (result.error) throw result.error;
    assert.equal(result.status, 0, `${command} ${args.join(" ")}`);
}
function digest(directory) {
    const hash = createHash("sha256");
    function visit(dir) {
        for (const entry of readdirSync(dir, { withFileTypes: true }).sort(
            (a, b) => a.name.localeCompare(b.name),
        )) {
            const file = path.join(dir, entry.name);
            if (entry.isDirectory()) visit(file);
            else
                hash.update(path.relative(directory, file)).update(
                    readFileSync(file),
                );
        }
    }
    visit(directory);
    return hash.digest("hex");
}
const temp = mkdtempSync(path.join(tmpdir(), "of-openapi-export-"));
try {
    for (const directory of ["first", "second"])
        run("cargo", [
            "run",
            "-p",
            "unified-server",
            "--example",
            "export-openapi",
            "--",
            path.join(temp, directory),
        ]);
    assert.equal(
        digest(path.join(temp, "first")),
        digest(path.join(temp, "second")),
        "exports must be byte-identical",
    );
    assert.equal(
        digest(path.join(temp, "first")),
        digest(path.join(root, "openapi")),
        "committed contracts must match production definitions",
    );
    for (const file of readdirSync(path.join(root, "openapi"))) {
        const doc = JSON.parse(readFileSync(path.join(root, "openapi", file)));
        const ids = new Set();
        for (const [route, item] of Object.entries(doc.paths))
            for (const operation of Object.values(item)) {
                assert.ok(
                    operation.operationId,
                    `${file}: ${route} operation ID`,
                );
                assert.ok(
                    !ids.has(operation.operationId),
                    `${file}: duplicate operation ID`,
                );
                ids.add(operation.operationId);
                for (const parameter of operation.parameters || [])
                    if (parameter.in === "path")
                        assert.ok(route.includes(`{${parameter.name}}`));
                for (const name of route.matchAll(/\{([^}]+)\}/g))
                    assert.ok(
                        operation.parameters?.some(
                            (p) => p.in === "path" && p.name === name[1],
                        ),
                    );
                for (const security of operation.security || [])
                    for (const scheme of Object.keys(security))
                        assert.ok(doc.components.securitySchemes[scheme]);
            }
    }
    const artifacts = [
        "packages/idp-client/src",
        "packages/management-client/src",
        "packages/storage-client/src",
        ...readdirSync(path.join(root, "openapi")).map(
            (file) => `clients/rust/${file.replace(/\.json$/, "")}`,
        ),
    ];
    const before = artifacts.map((dir) => digest(path.join(root, dir)));
    run(process.execPath, ["scripts/generate-openapi-client.mjs"]);
    assert.deepEqual(
        artifacts.map((dir) => digest(path.join(root, dir))),
        before,
        "both SDK families must regenerate without drift",
    );
    run(pnpm, [
        "--filter",
        "@aicacia/idp-client",
        "--filter",
        "@aicacia/management-client",
        "--filter",
        "@aicacia/storage-client",
        "run",
        "build:esm",
    ]);
    run(process.execPath, [
        "--test",
        ...readdirSync(path.join(root, "scripts"))
            .filter((file) =>
                /^(generate-openapi-client|generated-.*)\.test\.mjs$/.test(
                    file,
                ),
            )
            .map((file) => `scripts/${file}`),
    ]);
    run("cargo", ["test", "-p", "unified-server", "generated_"]);
    run("cargo", [
        "test",
        "-p",
        "unified-server",
        "builds_three_prefixed_services_with_one_endpoint",
    ]);
} finally {
    rmSync(temp, { recursive: true, force: true });
}
console.log("Offline deterministic contracts and both SDK families qualified");
