import { type ChildProcess, execFileSync, spawn } from "node:child_process";
import { once } from "node:events";
import { mkdir, mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import { request } from "node:https";
import { createServer } from "node:net";
import { resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath } from "node:url";
import { chromium, expect, type Page, test } from "@playwright/test";

const { E2E_CREATE_BACKEND_BIN, RUST_MIN_STACK } = process.env;
const owners = ["idp", "device", "storage"] as const;
const cwd = fileURLToPath(new URL("../", import.meta.url));
const username = "operator@setup.test";
const password = "a new installation password";

async function freePort() {
  const server = createServer().listen(0, "127.0.0.1");
  await once(server, "listening");
  const address = server.address();
  server.close();
  await once(server, "close");
  if (!address || typeof address === "string") throw new Error("No loopback port");
  return address.port;
}

function run(command: string, args: string[], env: NodeJS.ProcessEnv = process.env) {
  return execFileSync(command, args, { cwd, env, stdio: ["ignore", "pipe", "pipe"] })
    .toString()
    .trim();
}

async function tlsFixture(root: string) {
  const files = (name: string) => resolve(root, name);
  run("openssl", [
    "req",
    "-x509",
    "-newkey",
    "ec",
    "-pkeyopt",
    "ec_paramgen_curve:P-256",
    "-nodes",
    "-days",
    "1",
    "-subj",
    "/CN=Create E2E CA",
    "-keyout",
    files("ca.key"),
    "-out",
    files("ca.pem"),
  ]);
  run("openssl", [
    "req",
    "-new",
    "-newkey",
    "ec",
    "-pkeyopt",
    "ec_paramgen_curve:P-256",
    "-nodes",
    "-subj",
    "/CN=unified.localhost",
    "-keyout",
    files("tls.key"),
    "-out",
    files("tls.csr"),
  ]);
  await writeFile(
    files("tls.ext"),
    "basicConstraints=CA:FALSE\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:of.localhost,DNS:unified.localhost,DNS:*.of.localhost,DNS:*.unified.localhost\n"
  );
  run("openssl", [
    "x509",
    "-req",
    "-in",
    files("tls.csr"),
    "-CA",
    files("ca.pem"),
    "-CAkey",
    files("ca.key"),
    "-CAcreateserial",
    "-days",
    "1",
    "-extfile",
    files("tls.ext"),
    "-out",
    files("tls.pem"),
  ]);
  const home = files("browser-home");
  const nssDb = resolve(home, ".pki/nssdb");
  await mkdir(nssDb, { recursive: true });
  run("certutil", ["-N", "-d", `sql:${nssDb}`, "--empty-password"]);
  run("certutil", [
    "-A",
    "-d",
    `sql:${nssDb}`,
    "-n",
    "Create E2E CA",
    "-t",
    "C,,",
    "-i",
    files("ca.pem"),
  ]);
  return { home, ca: files("ca.pem"), key: files("tls.key"), certificate: files("tls.pem") };
}

function status(url: string, ca: Buffer): Promise<number> {
  return new Promise((done) => {
    const req = request(url, { ca, timeout: 1_000 }, (res) => {
      res.resume();
      done(res.statusCode ?? 0);
    });
    req.on("error", () => done(0));
    req.on("timeout", () => req.destroy());
    req.end();
  });
}

async function stop(child: ChildProcess | undefined) {
  if (!child?.pid || child.exitCode !== null || child.signalCode !== null) return;
  const exited = once(child, "exit");
  process.kill(-child.pid, "SIGTERM");
  if (await Promise.race([exited.then(() => false), delay(10_000, true, { ref: false })])) {
    process.kill(-child.pid, "SIGKILL");
    await exited;
  }
}

async function code(root: string, owner: string) {
  const path = resolve(root, `${owner}.setup-host/setup-admission/state`);
  const value = (await readFile(path, "utf8")).split("\n")[1];
  if (!value || !/^[a-f0-9]{64}$/.test(value) || (await stat(path)).mode & 0o077) {
    throw new Error("Owner setup code not available through protected local management");
  }
  return value;
}

async function fillCodes(page: Page, root: string) {
  for (const owner of owners) {
    await page.getByLabel(`${owner} host setup code`).fill(await code(root, owner));
  }
}

test("protected Create resumes after interruption and keeps the new identity after restart", async () => {
  await mkdir(resolve(cwd, ".tmp"), { recursive: true });
  const root = await mkdtemp(resolve(cwd, ".tmp/c"));
  const data = resolve(root, "d");
  let proxy: ChildProcess | undefined;
  let backend: ChildProcess | undefined;
  let frontend: ChildProcess | undefined;
  try {
    const tls = await tlsFixture(root);
    const ca = await readFile(tls.ca);
    const port = await freePort();
    const env = { ...process.env };
    for (const name of Object.keys(env)) {
      if (name.startsWith("PUBLIC_") || name.startsWith("PORTLESS_") || name === "PORT") {
        delete env[name];
      }
    }
    Object.assign(env, {
      PORTLESS_STATE_DIR: resolve(root, "proxy"),
      PORTLESS_PORT: String(port),
      PORTLESS_SYNC_HOSTS: "0",
      SSL_CERT_FILE: tls.ca,
      NODE_EXTRA_CA_CERTS: tls.ca,
      RUST_MIN_STACK: RUST_MIN_STACK ?? "67108864",
    });
    const launch = (args: string[]) =>
      spawn("pnpm", ["exec", "portless", ...args], {
        cwd,
        env,
        detached: true,
        stdio: "inherit",
      });
    proxy = launch([
      "proxy",
      "start",
      "--foreground",
      "--port",
      String(port),
      "--cert",
      tls.certificate,
      "--key",
      tls.key,
    ]);
    await expect.poll(() => status(`https://unified.localhost:${port}`, ca)).not.toBe(0);
    const base = run("pnpm", ["exec", "portless", "get", "unified"], env);
    const app = run("pnpm", ["exec", "portless", "get", "of"], env);
    const idp = `${base}/idp`;
    const binary = resolve(cwd, E2E_CREATE_BACKEND_BIN ?? "target/debug/unified-server");
    const startBackend = async () => {
      const port = await freePort();
      backend = launch([
        "run",
        "--name",
        "unified",
        "--app-port",
        String(port),
        binary,
        "setup",
        "--data-dir",
        data,
        "--origin",
        app,
      ]);
      await expect.poll(() => status(`${idp}/setup/owner/status`, ca)).toBe(200);
      return port;
    };
    const firstPort = await startBackend();
    frontend = launch([
      "run",
      "--name",
      "of",
      "pnpm",
      "--filter",
      "@ofsdk/app",
      "exec",
      "vite",
      "dev",
    ]);
    await expect.poll(() => status(app, ca)).toBe(200);
    const browser = await chromium.launch({ env: { ...process.env, HOME: tls.home } });
    try {
      const page = await browser.newPage();
      await page.goto(app);
      await expect(page.getByRole("heading", { name: "Set up this Node" })).toBeVisible();
      await expect(
        page.getByRole("heading", { name: "Create the first Installation" })
      ).toBeVisible();
      for (const owner of owners) {
        await expect(stat(resolve(data, owner))).rejects.toMatchObject({ code: "ENOENT" });
      }
      const intent = {
        purpose: "create",
        operation_nonce: "1a1577e501624de183f6127df454d37c",
        administrator: username,
        node: "My Node",
        idp,
        issuer: idp,
        device: `${base}/device`,
        storage: `${base}/storage`,
      };
      const submitIdp = (ownerCode?: string) =>
        page.evaluate(
          async ({ idp, intent, password, ownerCode }) => {
            const response = await fetch(`${idp}/setup/owner/create`, {
              method: "POST",
              headers: {
                "content-type": "application/json",
                ...(ownerCode && { authorization: `Setup ${ownerCode}` }),
              },
              credentials: "omit",
              body: JSON.stringify({ ...intent, password }),
            });
            return response.status;
          },
          { idp, intent, password, ownerCode }
        );
      expect(await submitIdp()).toBe(401);
      expect(await submitIdp(await code(data, "idp"))).toBe(202);
      await page.goto(`${app}/signin`);
      await expect(page).toHaveURL(`${app}/setup/installation`);
      await stop(backend);
      expect(await startBackend()).not.toBe(firstPort);
      await page.reload();
      await fillCodes(page, data);
      await page.getByRole("button", { name: "Recover exact operation across owners" }).click();
      await expect(page.getByLabel("Administrator username")).toHaveValue(username);
      await fillCodes(page, data);
      await page.getByLabel("IdP administrator password").fill(password);
      await page.getByRole("button", { name: "Retry exact Create across owners" }).click();
      await expect(
        page.getByText("Accepted by each incomplete owner; not complete.", { exact: false })
      ).toBeVisible();
      const waitForReady = async () => {
        for (const owner of owners) {
          await expect
            .poll(
              () =>
                page.evaluate(
                  async ({ base, owner }) => {
                    const res = await fetch(`${base}/${owner}/setup/owner/status`, {
                      cache: "no-store",
                    });
                    return res.ok && (await res.json()).ready === true;
                  },
                  { base, owner }
                ),
              { timeout: 60_000 }
            )
            .toBe(true);
        }
      };
      await waitForReady();
      await page.getByRole("button", { name: "Observe all owners and verify readiness" }).click();
      await page.getByRole("link", { name: "Continue to sign in" }).click();
      await expect(page).toHaveURL(`${app}/signin`);
      const signIn = async (name: string, credential: string) => {
        await page.getByLabel("Username", { exact: true }).fill(name);
        await page.getByLabel("Password", { exact: true }).fill(credential);
        const response = page.waitForResponse(
          (res) =>
            new URL(res.url()).pathname === "/idp/oauth2/token" && res.request().method() === "POST"
        );
        await page.getByRole("button", { name: "Sign in", exact: true }).click();
        return (await response).ok();
      };
      expect(await signIn("admin@local.test", "password")).toBe(false);
      expect(await signIn(username, password)).toBe(true);
      await expect(page).not.toHaveURL(`${app}/signin`);
      const evidence = () =>
        page.evaluate(
          async (idp) => (await fetch(`${idp}/setup/owner/public`, { cache: "no-store" })).json(),
          idp
        );
      const ready = await evidence();
      expect(ready.operation.operation_nonce).toBe(intent.operation_nonce);
      await stop(backend);
      await startBackend();
      await waitForReady();
      await page.goto(`${app}/signin`);
      await expect(page.getByRole("button", { name: "Sign in", exact: true })).toBeVisible();
      expect((await evidence()).facts).toEqual(ready.facts);
      expect(await signIn(username, password)).toBe(true);
    } finally {
      await browser.close();
    }
  } finally {
    await Promise.all([stop(backend), stop(frontend), stop(proxy)]);
    await rm(root, { recursive: true, force: true });
  }
});
