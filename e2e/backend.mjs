import { spawn } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath } from "node:url";

async function freePorts(count) {
  const servers = Array.from({ length: count }, () => createServer());
  try {
    await Promise.all(
      servers.map(
        (server) =>
          new Promise((resolve, reject) => {
            server.once("error", reject);
            server.listen(0, "127.0.0.1", resolve);
          })
      )
    );
    return servers.map((server) => server.address().port);
  } finally {
    await Promise.all(
      servers.map((server) => new Promise((resolve) => server.close(() => resolve())))
    );
  }
}

export async function freePort() {
  return (await freePorts(1))[0];
}

function runnerPort(name, fallback) {
  const port = Number(process.env[name] ?? fallback);
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new Error(`${name} must be an integer between 1 and 65535`);
  }
  process.env[name] = String(port);
  return port;
}

export const root = fileURLToPath(new URL("../", import.meta.url));
const allocatedPorts = await freePorts(3);
const backendPort = runnerPort("E2E_BACKEND_PORT", allocatedPorts[0]);
const appPort = runnerPort("E2E_APP_PORT", allocatedPorts[1]);
const passwordManagerPort = runnerPort("E2E_PASSWORD_MANAGER_PORT", allocatedPorts[2]);
export const backendURL = `http://127.0.0.1:${backendPort}`;
export const appURL = `http://127.0.0.1:${appPort}`;
export const passwordManagerURL = `http://127.0.0.1:${passwordManagerPort}`;

export async function startBackend({
  port = 0,
  timeoutMs = Number(process.env.E2E_START_TIMEOUT_MS ?? 180000),
  binary = process.env.E2E_BACKEND_BIN,
  command = binary ?? "cargo",
  args,
  env = {},
} = {}) {
  if (!Number.isFinite(timeoutMs) || timeoutMs <= 0) {
    throw new Error("E2E_START_TIMEOUT_MS must be a positive number");
  }
  const probe = createServer();
  await new Promise((resolve, reject) => {
    probe.once("error", (error) =>
      reject(
        error.code === "EADDRINUSE"
          ? new Error(`Backend port ${port} is already in use`, {
              cause: error,
            })
          : error
      )
    );
    probe.listen(port, "127.0.0.1", resolve);
  });
  port = probe.address().port;
  await new Promise((resolve, reject) =>
    probe.close((error) => (error ? reject(error) : resolve()))
  );
  console.info(`Starting backend ${command} on port ${port} (launcher PID ${process.pid})`);
  const dataDir = await mkdtemp(join(tmpdir(), "of-e2e-"));
  const baseURL = `http://127.0.0.1:${port}`;
  const child = spawn(
    command,
    args ?? [
      ...(binary ? [] : ["run", "--locked", "-p", "unified-server", "--"]),
      "seeded",
      "--data-dir",
      dataDir,
      "--port",
      String(port),
      "--origin",
      appURL,
      "--password-manager-origin",
      passwordManagerURL,
    ],
    {
      cwd: root,
      detached: true,
      stdio: "inherit",
      env: {
        ...process.env,
        RUST_MIN_STACK: process.env.RUST_MIN_STACK ?? "67108864",
        ...env,
        E2E_DATA_DIR: dataDir,
        E2E_PORT: String(port),
      },
    }
  );
  let failure;
  let exited = false;
  const closed = new Promise((resolve) => {
    child.once("error", (error) => {
      failure = error;
    });
    child.once("close", (code, signal) => {
      exited = true;
      failure ??= new Error(`Backend exited: ${signal ?? code}`);
      resolve();
    });
  });
  const kill = (signal) => {
    if (!child.pid) return;
    try {
      process.kill(-child.pid, signal);
    } catch (error) {
      if (error.code !== "ESRCH") throw error;
    }
  };
  let stopping;
  const stop = () =>
    (stopping ??= (async () => {
      try {
        kill("SIGTERM");
        await Promise.race([closed, delay(2000)]);
        // Cargo can exit before its server; kill the entire process group.
        kill("SIGKILL");
        await closed;
      } finally {
        process.off("SIGINT", interrupt);
        process.off("SIGTERM", terminate);
        process.off("exit", onExit);
        await rm(dataDir, { recursive: true, force: true });
      }
    })());
  const interrupt = () => {
    void stop().finally(() => process.exit(130));
  };
  const terminate = () => {
    void stop().finally(() => process.exit(143));
  };
  const onExit = () => kill("SIGKILL");
  process.once("SIGINT", interrupt);
  process.once("SIGTERM", terminate);
  process.once("exit", onExit);
  const pending = new Set(["idp", "device", "storage", "management"]);
  const deadline = Date.now() + timeoutMs;
  try {
    while (pending.size) {
      if (failure || exited) throw failure;
      if (Date.now() >= deadline) {
        throw new Error(
          `Backend health timeout: ${[...pending].map((owner) => `${baseURL}/${owner}/health`).join(", ")}`
        );
      }
      await Promise.all(
        [...pending].map(async (owner) => {
          try {
            const response = await fetch(`${baseURL}/${owner}/health`, {
              signal: AbortSignal.timeout(Math.max(1, Math.min(500, deadline - Date.now()))),
            });
            if (response.ok) pending.delete(owner);
            await response.body?.cancel();
          } catch {
            /* Retry until the shared startup deadline. */
          }
        })
      );
      if (pending.size) await delay(100);
    }
    if (failure) throw failure;
    return { baseURL, dataDir, pid: child.pid, stop };
  } catch (error) {
    await stop();
    throw error;
  }
}

export default async function globalSetup() {
  const backend = await startBackend({ port: backendPort });
  return backend.stop;
}
