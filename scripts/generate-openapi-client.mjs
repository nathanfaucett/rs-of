import { spawnSync } from "node:child_process";
import {
    cpSync,
    existsSync,
    mkdirSync,
    mkdtempSync,
    readFileSync,
    readdirSync,
    renameSync,
    rmSync,
    writeFileSync,
} from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "..");
const destinations = {
    "owner-setup": ["idp", "admission"],
    idp: ["idp", ""],
    management: ["management", ""],
    storage: ["storage", "generated"],
    "idp-setup": ["idp", "setup"],
    "idp-independent": ["idp", "independent"],
    "node-join": ["management", "node-join"],
    "independent-idp": ["idp", "owner"],
    "independent-management": ["management", "owner"],
    "independent-storage": ["storage", "owner"],
};
const selected = process.argv[2];
if (selected && !destinations[selected])
    throw new Error("Unknown OpenAPI document");
const names = selected ? [selected] : Object.keys(destinations);
const stage = mkdtempSync(path.join(root, ".openapi-generation-"));
const pnpm = process.env.npm_execpath || "pnpm";
function run(command, args, cwd = root) {
    const result = spawnSync(command, args, {
        cwd,
        stdio: "inherit",
        timeout: 180000,
    });
    if (result.error) throw result.error;
    if (result.status !== 0)
        throw new Error(`${command} failed (${result.status})`);
}
function generate(name, language, output) {
    const spec = process.env.OPENAPI_SPEC_FILE
        ? path.resolve(process.env.OPENAPI_SPEC_FILE)
        : path.join(root, "openapi", `${name}.json`);
    const document = JSON.parse(readFileSync(spec, "utf8"));
    if (
        !document.openapi ||
        !document.paths ||
        !Object.keys(document.paths).length
    )
        throw new Error("Invalid or empty OpenAPI document");
    run(
        pnpm,
        ["exec", "openapi-generator-cli", "validate", "-i", spec],
        path.join(root, "packages/idp-client"),
    );
    const args = [
        "exec",
        "openapi-generator-cli",
        "generate",
        "-i",
        spec,
        "-g",
        language,
        "-o",
        output,
        "--global-property=apiDocs=false,modelDocs=false,apiTests=false,modelTests=false",
    ];
    if (language === "rust")
        args.push(
            "-c",
            path.join(root, "scripts/openapi-templates/config.json"),
            "-t",
            path.join(root, "scripts/openapi-templates"),
            `--additional-properties=packageName=of-${name}-client,useSingleRequestParameter=true,useChrono=false`,
        );
    else
        args.push(
            "--type-mappings=DateTime=Date",
            "--additional-properties=supportsES6=true,typescriptThreePlus=true,withInterfaces=true,useSingleRequestParameter=true,importFileExtension=.js",
        );
    run(pnpm, args, path.join(root, "packages/idp-client"));
    for (const entry of readdirSync(output)) {
        if (
            entry !== "src" &&
            !(language === "rust" && entry === "Cargo.toml") &&
            !(
                language === "typescript-fetch" &&
                ["apis", "models", "index.ts", "runtime.ts"].includes(entry)
            )
        )
            rmSync(path.join(output, entry), { recursive: true, force: true });
    }
}
const replacements = [];
try {
    const owners = [...new Set(names.map((name) => destinations[name][0]))];
    for (const owner of owners) {
        const source = path.join(root, `packages/${owner}-client/src`);
        const staged = path.join(stage, `packages/${owner}-client/src`);
        cpSync(source, staged, { recursive: true });
        replacements.push([staged, source]);
    }
    const rust = path.join(stage, "rust");
    mkdirSync(rust);
    for (const name of Object.keys(destinations)) {
        if (!names.includes(name))
            cpSync(
                path.join(root, "clients/rust", name),
                path.join(rust, name),
                { recursive: true },
            );
    }
    cpSync(
        path.join(root, "clients/rust/Cargo.lock"),
        path.join(rust, "Cargo.lock"),
    );
    for (const name of names) {
        const [owner, suffix] = destinations[name];
        const ts = path.join(stage, "typescript", name);
        generate(name, "typescript-fetch", ts);
        const target = path.join(stage, `packages/${owner}-client/src`, suffix);
        if (suffix) rmSync(target, { recursive: true, force: true });
        else
            for (const entry of [
                "apis",
                "models",
                "runtime.ts",
                "index.ts",
                ".openapi-generator",
                ".openapi-generator-ignore",
            ])
                rmSync(path.join(target, entry), {
                    recursive: true,
                    force: true,
                });
        run(pnpm, [
            "exec",
            "biome",
            "format",
            "--write",
            "--format-with-errors=false",
            ts,
        ]);
        cpSync(ts, target, { recursive: true });
        generate(name, "rust", path.join(rust, name));
        replacements.push([
            path.join(rust, name),
            path.join(root, "clients/rust", name),
        ]);
    }
    writeFileSync(
        path.join(rust, "Cargo.toml"),
        `[workspace]\nresolver = "3"\nmembers = ${JSON.stringify(Object.keys(destinations))}\n`,
    );
    run("cargo", [
        "fmt",
        "--manifest-path",
        path.join(rust, "Cargo.toml"),
        "--all",
    ]);
    run("cargo", [
        "check",
        "--manifest-path",
        path.join(rust, "Cargo.toml"),
        "--workspace",
        "--offline",
        "--locked",
    ]);
    for (const owner of owners) {
        const source = path.join(stage, `packages/${owner}-client/src`);
        run(
            pnpm,
            [
                "exec",
                "tsc",
                "--target",
                "es2020",
                "--module",
                "es2020",
                "--moduleResolution",
                "bundler",
                "--strict",
                "--skipLibCheck",
                "--outDir",
                path.join(stage, "build", owner),
                ...readdirSync(source)
                    .filter(
                        (file) =>
                            file.endsWith(".ts") && !file.endsWith(".test.ts"),
                    )
                    .map((file) => path.join(source, file)),
                ...names
                    .filter(
                        (name) =>
                            destinations[name][0] === owner &&
                            destinations[name][1],
                    )
                    .map((name) =>
                        path.join(source, destinations[name][1], "index.ts"),
                    ),
            ],
            path.join(root, "packages/idp-client"),
        );
    }
    const moved = [];
    try {
        for (const [from, to] of replacements) {
            mkdirSync(path.dirname(to), { recursive: true });
            const backup = path.join(stage, `backup-${moved.length}`);
            const existed = existsSync(to);
            if (existed) renameSync(to, backup);
            moved.push([to, backup, existed]);
            renameSync(from, to);
        }
    } catch (error) {
        for (const [to, backup, existed] of moved.reverse()) {
            rmSync(to, { recursive: true, force: true });
            if (existed) renameSync(backup, to);
        }
        throw error;
    }
} finally {
    rmSync(stage, { recursive: true, force: true });
}
