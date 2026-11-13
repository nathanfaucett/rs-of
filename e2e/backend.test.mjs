import assert from "node:assert/strict";
import { access } from "node:fs/promises";
import { createServer } from "node:net";
import { test } from "node:test";
import { startBackend } from "./backend.mjs";

async function freePort() {
  const server = createServer();
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address();
  await new Promise((resolve) => server.close(resolve));
  return port;
}

// This process fixture tests supervision only; domain E2E tests always run Rust.
const fixture = `
const http = require('node:http');
const fs = require('node:fs');
fs.writeFileSync(process.env.E2E_DATA_DIR + '/started', String(process.pid));
http.createServer((req, res) => {
  res.statusCode = req.url === '/management/health' && process.env.UNHEALTHY ? 503 : 200;
  res.end('ok');
}).listen(Number(process.env.E2E_PORT), '127.0.0.1');
`;

const options = async (env = {}) => ({
  port: await freePort(),
  command: process.execPath,
  args: ["-e", fixture],
  env,
  timeoutMs: 1000,
});

test("waits for all owners, isolates data, and stops the process before removing data", async () => {
  const first = await startBackend(await options());
  const second = await startBackend(await options());
  try {
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
  const config = await options({ UNHEALTHY: "1" });
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

test("reports spawn failure and removes temporary data", async () => {
  const config = await options();
  config.command = "/missing/e2e-command";
  await assert.rejects(startBackend(config), /ENOENT/);
});
