import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import test from "node:test";

function dependencies(args) {
  return execFileSync("cargo", ["tree", "--edges", "normal,build", "--prefix", "none", "--format", "{p}", ...args], { encoding: "utf8" });
}

test("unified defaults do not include the independent setup runtime", () => {
  assert.doesNotMatch(dependencies(["-p", "unified-server"]), /^setup v/m);
});

test("independent owners retain setup without enabling their CLI", () => {
  for (const owner of ["idp-server", "device-server", "storage-server"]) {
    assert.match(dependencies(["-p", owner, "--no-default-features", "--features", "independent"]), /^setup v/m);
  }
});
