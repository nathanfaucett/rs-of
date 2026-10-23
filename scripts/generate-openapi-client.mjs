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
    symlinkSync,
    writeFileSync,
} from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "..");
const destinations = {
    idp: ["idp", ""],
    management: ["management", ""],
    device: ["device", ""],
    storage: ["storage", "generated"],
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
function restoreUnionCompositions(document, language, output) {
    const schemas = document.components?.schemas || {};
    for (const [name, schema] of Object.entries(schemas)) {
        const reference = schema.allOf?.find(
            (part) => schemas[part.$ref?.split("/").at(-1)]?.oneOf,
        )?.$ref;
        if (!reference) continue;
        const parent = reference.split("/").at(-1);
        const properties = Object.assign(
            {},
            ...schema.allOf.map((part) => part.properties || {}),
        );
        const required = schema.allOf.flatMap((part) => part.required || []);
        const fields = Object.entries(properties).map(([field, property]) => {
            const model = property.$ref?.split("/").at(-1);
            if (
                (!model && property.type !== "string") ||
                !required.includes(field)
            )
                throw new Error(`Unsupported composed field ${name}.${field}`);
            return { field, model };
        });
        if (language === "rust") {
            const filename = name
                .replace(/([a-z])([A-Z])/g, "$1_$2")
                .toLowerCase();
            writeFileSync(
                path.join(output, "src/models", `${filename}.rs`),
                `// Generated from the canonical allOf/oneOf composition.\nuse crate::models;\nuse serde::{Deserialize, Serialize};\n#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]\npub struct ${name} {\n    #[serde(flatten)]\n    pub parameters: models::${parent},\n${fields.map(({ field, model }) => `    #[serde(rename = "${field}")]\n    pub ${field === "use" ? "r#use" : field}: ${model ? `models::${model}` : "String"},`).join("\n")}\n}\n`,
            );
        } else {
            const imports = [
                parent,
                ...fields.flatMap(({ model }) => (model ? [model] : [])),
            ];
            writeFileSync(
                path.join(output, "models", `${name}.ts`),
                `// Generated from the canonical allOf/oneOf composition.\n${imports.map((model) => `import { type ${model}, ${model}FromJSON, ${model}ToJSON } from "./${model}.js";`).join("\n")}\nexport type ${name} = ${parent} & { ${fields.map(({ field, model }) => `${field}: ${model || "string"}`).join("; ")} };\nexport function instanceOf${name}(value: object): value is ${name} { return ${JSON.stringify(required)}.every(key => key in value); }\nexport function ${name}FromJSON(json: any): ${name} { return ${name}FromJSONTyped(json, false); }\nexport function ${name}FromJSONTyped(json: any, ignoreDiscriminator: boolean): ${name} {\n    if (json == null) return json;\n    const parameters = ${parent}FromJSON(json);\n    if (!parameters || !Object.keys(parameters).length) throw new Error("Invalid ${parent}");\n    return { ...parameters, ${fields.map(({ field, model }) => `${field}: ${model ? `${model}FromJSON(json["${field}"])` : `json["${field}"]`}`).join(", ")} };\n}\nexport function ${name}ToJSON(value: any): any { return ${name}ToJSONTyped(value); }\nexport function ${name}ToJSONTyped(value?: ${name} | null, ignoreDiscriminator = false): any {\n    if (value == null) return value;\n    return { ...${parent}ToJSON(value), ${fields.map(({ field, model }) => `${field}: ${model ? `${model}ToJSON(value["${field}"])` : `value["${field}"]`}`).join(", ")} };\n}\n`,
            );
        }
    }
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
    if (language === "typescript-fetch") {
        if (document.paths["/"]) {
            const apis = path.join(output, "apis");
            for (const file of readdirSync(apis).filter((file) =>
                file.endsWith("Api.ts"),
            )) {
                const target = path.join(apis, file);
                writeFileSync(
                    target,
                    readFileSync(target, "utf8").replace(
                        "let urlPath = `/`;",
                        "let urlPath = ``;",
                    ),
                );
            }
        }
        const schemas = document.components?.schemas || {};
        const grants =
            schemas.TokenRequest?.oneOf
                ?.flatMap((variant) => variant.allOf || [])
                .flatMap((part) => part.properties?.grant_type?.enum || []) ||
            [];
        if (grants.length) {
            const api = path.join(output, "apis/DefaultApi.ts");
            writeFileSync(
                api,
                readFileSync(api, "utf8").replace(
                    "export const TokenGrantTypeEnum = {",
                    `export const TokenGrantTypeEnum = {\n${grants.map((grant) => `${JSON.stringify(grant)}: ${JSON.stringify(grant)},`).join("\n")}`,
                ),
            );
        }
        const models = path.join(output, "models");
        for (const file of readdirSync(models).filter((file) =>
            file.endsWith(".ts"),
        )) {
            const target = path.join(models, file);
            let source = readFileSync(target, "utf8");
            source = source.replace(
                /instanceOf(\w+)\(json\)/g,
                (match, name) =>
                    schemas[name]?.required?.length
                        ? `${JSON.stringify(schemas[name].required)}.every(key => key in json)`
                        : match,
            );
            const name = file.replace(/\.ts$/, "");
            if (schemas[name]?.additionalProperties === false) {
                const keys = JSON.stringify(
                    Object.keys(schemas[name].properties || {}),
                );
                source = source.replace(
                    new RegExp(
                        `(export function ${name}FromJSONTyped\\([^]*?\\)\\s*:\\s*[^\\{]+\\{)`,
                    ),
                    `$1\n    if (json && typeof json === "object" && Object.keys(json).some(key => !${keys}.includes(key))) throw new Error("Unexpected ${name} field");`,
                );
            }
            writeFileSync(target, source);
        }
    }
    if (language === "rust") {
        const closed = new Set(
            Object.entries(document.components?.schemas || {})
                .filter(([, schema]) => schema.additionalProperties === false)
                .map(([name]) => name),
        );
        const upgrades = new Set(
            Object.values(document.paths).flatMap((item) =>
                Object.values(item)
                    .filter((operation) => operation.responses?.["101"])
                    .map((operation) =>
                        operation.operationId.replace(
                            /[A-Z]/g,
                            (letter) => `_${letter.toLowerCase()}`,
                        ),
                    ),
            ),
        );
        const apis = path.join(output, "src/apis");
        for (const file of readdirSync(apis).filter((file) =>
            file.endsWith("_api.rs"),
        )) {
            const target = path.join(apis, file);
            let source = readFileSync(target, "utf8").replace(
                "#[derive(Clone, Debug)]\npub struct TokenParams",
                "#[derive(Clone, Debug, Default)]\npub struct TokenParams",
            );
            if (document.paths["/"])
                source = source.replace(
                    'format!("{}/", configuration.base_path)',
                    'format!("{}", configuration.base_path)',
                );
            source = source.replace(
                /pub async fn (\w+)\(([^{}]*?)\) -> Result<\(\), Error<(\w+)>> \{([\s\S]*?)    let resp = configuration.client.execute\(req\).await\?;/g,
                (operation, name, args, error, build) => {
                    if (!upgrades.has(name)) return operation;
                    const builder = `pub fn ${name}_request(${args}) -> Result<reqwest::Request, Error<${error}>> {${build}    Ok(req)\n}\n\n`;
                    return `${builder}pub async fn ${name}(${args}) -> Result<reqwest::Response, Error<${error}>> {\n    let req = ${name}_request(configuration, params)?;\n    let resp = configuration.client.execute(req).await?;`;
                },
            );
            for (const name of upgrades) {
                const start = source.indexOf(`pub async fn ${name}(`);
                if (start < 0) continue;
                const end = source.indexOf("\npub ", start + 1);
                const operation = source.slice(
                    start,
                    end < 0 ? undefined : end,
                );
                source = source.replace(
                    operation,
                    operation.replace(
                        /if [^\n]*false \{\n\s*Ok\(\(\)\)/,
                        "if status == reqwest::StatusCode::SWITCHING_PROTOCOLS {\n        Ok(resp)",
                    ),
                );
            }
            writeFileSync(target, source);
        }
        const models = path.join(output, "src/models");
        for (const file of readdirSync(models)) {
            const target = path.join(models, file);
            const source = readFileSync(target, "utf8");
            writeFileSync(
                target,
                source.replace(/pub struct (\w+) \{/g, (declaration, name) =>
                    closed.has(name)
                        ? `#[serde(deny_unknown_fields)]\n${declaration}`
                        : declaration,
                ),
            );
        }
    }
    restoreUnionCompositions(document, language, output);
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
        symlinkSync(
            path.join(root, `packages/${owner}-client/node_modules`),
            path.join(stage, `packages/${owner}-client/node_modules`),
            "dir",
        );
        replacements.push([staged, source]);
    }
    const rust = path.join(stage, "rust");
    mkdirSync(rust);
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
            path.join(root, "crates", `${name}-client`),
        ]);
    }
    for (const name of names) {
        const source = path.join(rust, name, "src");
        run("rustfmt", ["--edition", "2024", path.join(source, "lib.rs")]);
    }
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
        run("cargo", [
            "fmt",
            "--check",
            ...names.flatMap((name) => ["-p", `of-${name}-client`]),
        ]);
        run("cargo", ["fetch", "--locked"]);
        run("cargo", [
            "check",
            "--locked",
            "--offline",
            ...names.flatMap((name) => ["-p", `of-${name}-client`]),
        ]);
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
