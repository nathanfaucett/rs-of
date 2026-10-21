import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import test from "node:test";

for (const owner of ["idp", "management", "storage"]) {
    const manifest = new URL(
        "../packages/" + owner + "-client/package.json",
        import.meta.url,
    );
    const pkg = JSON.parse(readFileSync(manifest, "utf8"));
    const require = createRequire(manifest);
    for (const entry of Object.keys(pkg.exports)) {
        if (entry === "./package.json") continue;
        const specifier = pkg.name + (entry === "." ? "" : entry.slice(1));
        test("built export resolves and loads: " + specifier, async () => {
            const module = await import(require.resolve(specifier));
            assert.ok(Object.keys(module).length > 0);
        });
    }
}
