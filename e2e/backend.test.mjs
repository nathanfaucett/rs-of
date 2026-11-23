import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { access, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { promisify } from "node:util";
import { appURL, backendURL, freePort, passwordManagerURL, startBackend } from "./backend.mjs";

const execFileAsync = promisify(execFile);

// This process fixture tests supervision only; domain E2E tests always run Rust.
const fixture = `
const http = require('node:http');
const fs = require('node:fs');
if (process.argv[2] === 'local') {
  const assert = require('node:assert/strict');
  const args = process.argv.slice(2);
  assert.equal(args[args.indexOf('--data-dir') + 1], process.env.E2E_DATA_DIR);
  assert.equal(args[args.indexOf('--port') + 1], process.env.E2E_PORT);
  assert.equal(args[args.indexOf('--origin') + 1], ${JSON.stringify(appURL)});
  assert.equal(args[args.indexOf('--password-manager-origin') + 1], ${JSON.stringify(passwordManagerURL)});
  assert.ok(!args.includes('--no-tls'));
  assert.equal(process.env.RUST_MIN_STACK, '67108864');
}
fs.writeFileSync(process.env.E2E_DATA_DIR + '/started', String(process.pid));
http.createServer((req, res) => {
  res.statusCode = req.url === '/management/health' && process.env.UNHEALTHY ? 503 : 200;
  res.end('ok');
}).listen(Number(process.env.E2E_PORT), '127.0.0.1');
`;

const options = async (env = {}) => ({
  command: process.execPath,
  args: ["-e", fixture],
  env,
  timeoutMs: 1000,
});

test("allocates runner URLs with distinct dynamic ports", () => {
  const urls = [backendURL, appURL, passwordManagerURL];
  assert.equal(new Set(urls.map((url) => new URL(url).port)).size, 3);
  for (const url of urls) {
    assert.ok(!["1355", "5173", "5174"].includes(new URL(url).port));
  }
});

test("shares explicit runner ports with subprocesses and validates them", async () => {
  const module = JSON.stringify(new URL("./backend.mjs", import.meta.url).href);
  const code = `import { backendURL, appURL, passwordManagerURL } from ${module}; console.log(JSON.stringify([backendURL, appURL, passwordManagerURL]));`;
  const env = {
    ...process.env,
    E2E_BACKEND_PORT: "41001",
    E2E_APP_PORT: "41002",
    E2E_PASSWORD_MANAGER_PORT: "41003",
  };
  const { stdout } = await execFileAsync(process.execPath, ["--input-type=module", "-e", code], {
    env,
  });
  assert.deepEqual(JSON.parse(stdout), [
    "http://127.0.0.1:41001",
    "http://127.0.0.1:41002",
    "http://127.0.0.1:41003",
  ]);
  await assert.rejects(
    execFileAsync(process.execPath, ["--input-type=module", "-e", code], {
      env: { ...env, E2E_BACKEND_PORT: "0" },
    }),
    /E2E_BACKEND_PORT must be an integer/
  );
});

test("waits for all owners, allocates dynamic ports, isolates data, and stops the process before removing data", async () => {
  const first = await startBackend(await options());
  const second = await startBackend(await options());
  try {
    assert.notEqual(first.baseURL, second.baseURL);
    assert.notEqual(new URL(first.baseURL).port, "1355");
    assert.notEqual(first.dataDir, second.dataDir);
    await access(`${first.dataDir}/started`);
    for (const owner of ["idp", "device", "storage", "management"]) {
      assert.equal((await fetch(`${first.baseURL}/${owner}/health`)).status, 200);
    }
  } finally {
    await Promise.all([first.stop(), second.stop()]);
  }
  await first.stop();
  assert.throws(() => process.kill(first.pid, 0), { code: "ESRCH" });
  await assert.rejects(access(first.dataDir), { code: "ENOENT" });
});

test("times out when any owner is unhealthy and cleans up the failed process", async () => {
  const config = {
    ...(await options({ UNHEALTHY: "1" })),
    port: await freePort(),
  };
  await assert.rejects(startBackend(config), /management.*health/);
  await assert.rejects(fetch(`http://127.0.0.1:${config.port}/idp/health`));
});

test("refuses an occupied port instead of accepting another backend's health", async () => {
  const existing = await startBackend(await options());
  try {
    await assert.rejects(
      startBackend({
        ...(await options()),
        port: Number(new URL(existing.baseURL).port),
        args: ["-e", "setTimeout(() => {}, 10000)"],
      }),
      /already in use/
    );
  } finally {
    await existing.stop();
  }
});

test("reports a child exit without waiting for the health deadline", async () => {
  const config = await options();
  config.args = ["-e", "process.exit(23)"];
  await assert.rejects(startBackend(config), /exited.*23/);
});

test("starts a prebuilt executable with the local CLI and an isolated data directory", async () => {
  const directory = await mkdtemp(join(tmpdir(), "of-launcher-test-"));
  const binary = join(directory, "server.cjs");
  await writeFile(binary, `#!${process.execPath}\n${fixture}`, { mode: 0o755 });
  try {
    const backend = await startBackend({
      binary,
      port: await freePort(),
      timeoutMs: 2000,
    });
    try {
      await access(join(backend.dataDir, "started"));
    } finally {
      await backend.stop();
    }
    await assert.rejects(access(backend.dataDir), { code: "ENOENT" });
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("uses an explicit prebuilt binary without falling back to Cargo", async () => {
  await assert.rejects(
    startBackend({
      binary: "/missing/e2e-binary",
      port: await freePort(),
      timeoutMs: 500,
    }),
    /ENOENT/
  );
});

test("reports spawn failure and removes temporary data", async () => {
  const config = await options();
  config.command = "/missing/e2e-command";
  await assert.rejects(startBackend(config), /ENOENT/);
});
