import { type ChildProcess, execFileSync, spawn } from "node:child_process";
import { generateKeyPairSync } from "node:crypto";
import { once } from "node:events";
import { mkdir, mkdtemp, readdir, readFile, rm, stat, writeFile } from "node:fs/promises";
import { request } from "node:https";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath } from "node:url";
import { chromium, expect, type Page, test } from "@playwright/test";

import {
  localRecord,
  localSecret,
  localVault,
  masterPassword,
  passwordManagerOpensLegacyVault,
  passwordManagerSignin,
  passwordManagerWhileWaiting,
} from "./password-manager-setup";

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
  return execFileSync(command, args, {
    cwd,
    env,
    stdio: ["ignore", "pipe", "pipe"],
  })
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
    "basicConstraints=CA:FALSE\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:of.localhost,DNS:unified.localhost,DNS:password-manager.localhost,DNS:*.of.localhost,DNS:*.unified.localhost,DNS:*.password-manager.localhost,DNS:remote.localhost,DNS:*.remote.localhost,DNS:remote-ui.localhost,DNS:*.remote-ui.localhost\n"
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
  return {
    home,
    ca: files("ca.pem"),
    key: files("tls.key"),
    certificate: files("tls.pem"),
  };
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

async function code(path: string, raw = false) {
  const text = await readFile(path, "utf8");
  const value = raw ? text : text.split("\n")[1];
  if (!value || !/^[a-f0-9]{64}$/.test(value) || (await stat(path)).mode & 0o077) {
    throw new Error("Setup code not available through protected local management");
  }
  return value;
}

async function launchNode(root: string) {
  const tls = await tlsFixture(root);
  const ca = await readFile(tls.ca);
  const port = await freePort();
  const data = resolve(root, "node");
  const env = { ...process.env };
  for (const name of Object.keys(env)) {
    if (
      name.startsWith("PUBLIC_") ||
      name.startsWith("PORTLESS_") ||
      name === "PORT" ||
      name === "RUST_MIN_STACK"
    ) {
      delete env[name];
    }
  }
  Object.assign(env, {
    PORTLESS_STATE_DIR: resolve(root, "proxy"),
    PORTLESS_PORT: String(port),
    PORTLESS_SYNC_HOSTS: "0",
    SSL_CERT_FILE: tls.ca,
    NODE_EXTRA_CA_CERTS: tls.ca,
  });
  let backend: ChildProcess | undefined;
  const children: ChildProcess[] = [];
  const launch = (command: string, args: string[], extraEnv?: NodeJS.ProcessEnv) => {
    const child = spawn(command, args, {
      cwd,
      env: { ...env, ...extraEnv },
      detached: true,
      stdio: "inherit",
    });
    children.push(child);
    return child;
  };
  const close = () => Promise.all(children.reverse().map(stop));
  try {
    launch("pnpm", [
      "exec",
      "portless",
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
    const url = (name: string) => run("pnpm", ["exec", "portless", "get", name], env);
    const base = url("unified");
    const app = url("of");
    const pm = url("password-manager");
    const ready = (url: string) =>
      expect.poll(() => status(url, ca), { timeout: 60_000 }).toBe(200);
    const startBackend = async () => {
      // Match the seeded runner's stack allowance for backend SQL workloads.
      const stack = { RUST_MIN_STACK: "67108864" };
      const { E2E_BACKEND_BIN: binary } = process.env;
      backend = binary
        ? launch(
            "pnpm",
            ["exec", "portless", "run", "--name", "unified", binary, "setup", "--data-dir", data],
            stack
          )
        : launch("just", ["unified", "--data-dir", data], stack);
      await ready(`${base}/setup/node`);
    };
    await startBackend();
    // Keep workspace .env overrides from selecting a developer's running Node.
    const nodeEnv = {
      PUBLIC_UNIFIED_ORIGIN: base,
      PUBLIC_IDP_BASE_URL: `${base}/idp`,
      PUBLIC_DEVICE_BASE_URL: `${base}/device`,
      PUBLIC_MANAGEMENT_BASE_URL: `${base}/management`,
      PUBLIC_STORAGE_BASE_URL: `${base}/storage`,
    };
    launch("pnpm", ["--filter", "@ofsdk/app", "dev"], {
      ...nodeEnv,
      PUBLIC_URL: app,
    });
    launch("pnpm", ["--filter", "@ofsdk/password-manager", "dev"], {
      ...nodeEnv,
      PUBLIC_URL: pm,
      PUBLIC_NODE_SETUP_URL: `${app}/setup`,
    });
    await ready(app);
    await ready(pm);
    const browser = await chromium.launch({
      env: { ...process.env, HOME: tls.home },
    });
    return {
      data,
      base,
      app,
      pm,
      ca,
      url,
      launch,
      ready,
      browser,
      restart: async () => {
        await stop(backend);
        await startBackend();
      },
      close: async () => {
        await browser.close();
        await close();
      },
    };
  } catch (error) {
    await close();
    throw error;
  }
}

const nodeTest = test.extend<{ node: Awaited<ReturnType<typeof launchNode>> }>({
  node: async ({ browserName }, use) => {
    expect(browserName).toBe("chromium");

    const root = await mkdtemp(resolve(tmpdir(), "of-first-run-"));
    try {
      const node = await launchNode(root);
      try {
        await use(node);
      } finally {
        await node.close();
      }
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  },
});

function descriptor(page: Page, base: string) {
  return page.evaluate(
    async (base) => (await fetch(`${base}/setup/node`, { cache: "no-store" })).json(),
    base
  );
}

async function select(page: Page, base: string, choice: "create" | "join", code?: string) {
  return page.evaluate(
    async ({ base, choice, code }) =>
      (
        await fetch(`${base}/setup/node/choice`, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            ...(code && { Authorization: `Setup ${code}` }),
          },
          body: JSON.stringify(choice),
        })
      ).status,
    { base, choice, code }
  );
}

async function choose(
  page: Page,
  node: { app: string; base: string; data: string },
  choice: "create" | "join"
) {
  await page.goto(node.app);
  await expect(page.getByRole("heading", { name: "Set up this Node" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Start a new Installation" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Join an existing Installation" })).toBeVisible();
  expect(await descriptor(page, node.base)).toEqual({
    state: "awaiting_choice",
  });
  expect(await select(page, node.base, choice)).toBe(401);
  for (const owner of owners) {
    await expect(stat(resolve(node.data, `${owner}.setup-host`))).rejects.toMatchObject({
      code: "ENOENT",
    });
  }
  const nodeCode = await code(resolve(node.data, "node-admission/state"), true);
  await page.getByLabel("Node setup code", { exact: true }).fill(nodeCode);
  await page
    .getByRole("button", {
      name: choice === "create" ? "Start a new Installation" : "Join an existing Installation",
    })
    .click();
  await expect(page).toHaveURL(
    `${node.app}/setup/${choice === "create" ? "installation" : "join"}`
  );
  expect(await select(page, node.base, choice === "create" ? "join" : "create", nodeCode)).toBe(
    409
  );
  return nodeCode;
}

// Reject only the local binding write, not vault history or the server transaction.
async function rejectNextBindingWrite(page: Page) {
  await page.evaluate(() => {
    const put = IDBObjectStore.prototype.put;
    IDBObjectStore.prototype.put = function (value, key) {
      if (
        this.name === "records" &&
        Array.isArray(key) &&
        key[0] === "ofdb:local:initial-restore"
      ) {
        IDBObjectStore.prototype.put = put;
        throw new DOMException("Injected binding write failure", "QuotaExceededError");
      }
      return put.call(this, value, key);
    };
  });
}

async function rejectStorageChanges(page: Page, keys: string[] | "all") {
  await page.evaluate((keys) => {
    const remove = Storage.prototype.removeItem;
    const set = Storage.prototype.setItem;
    const clear = Storage.prototype.clear;
    function reject(storage: Storage, key: string) {
      if (storage === localStorage && (keys === "all" || keys.includes(key)))
        throw new DOMException("Injected localStorage failure", "SecurityError");
    }
    Storage.prototype.removeItem = function (key) {
      reject(this, key);
      remove.call(this, key);
    };
    Storage.prototype.setItem = function (key, value) {
      reject(this, key);
      set.call(this, key, value);
    };
    Storage.prototype.clear = function () {
      reject(this, "");
      clear.call(this);
    };
  }, keys);
}

nodeTest(
  "default Create retains the Installation and supports guarded vault upload and restore",
  async ({ node }) => {
    const { app, base, browser, ca, data, pm } = node;
    const idp = `${base}/idp`;
    const storage = `${base}/storage`;
    const page = await browser.newPage();
    const pmPage = await browser.newPage();
    const vaultId = await passwordManagerWhileWaiting(pmPage, pm, base, app);
    const nodeCode = await choose(page, node, "create");
    await expect(page.getByLabel("Administrator username", { exact: true })).toBeVisible();
    await expect(page.getByLabel(/setup code/i)).toHaveCount(0);
    await expect.poll(() => descriptor(page, base)).toEqual({ state: "creating" });
    for (const owner of owners) {
      await expect(stat(resolve(data, owner))).rejects.toMatchObject({
        code: "ENOENT",
      });
    }

    const endpoint = `${base}/setup/node/create`;
    const submit = (origin: string, setupCode?: string) =>
      api(endpoint, ca, {
        method: "POST",
        headers: {
          Origin: origin,
          "Content-Type": "application/json",
          ...(setupCode && { Authorization: `Setup ${setupCode}` }),
        },
        body: JSON.stringify({ administrator: username, password }),
      });
    expect((await submit(app)).status).toBe(401);
    const denied = await submit(pm, nodeCode);
    expect(denied.status).toBe(403);
    expect(denied.origin).toBeUndefined();
    const intent = await api(`${endpoint}/intent`, ca, {
      method: "POST",
      headers: { Origin: app, Authorization: `Setup ${nodeCode}` },
    });
    expect(intent.status).toBe(204);

    const expectNoOwnerSetupRoutes = async () => {
      for (const owner of owners) {
        for (const [method, routes] of [
          ["GET", ["status", "public"]],
          ["POST", ["admission", "create", "intent", "cancel", "challenge", "register"]],
        ] as const) {
          for (const route of routes) {
            const path = `/${owner}/setup/owner/${route}`;
            const { status } = await api(`${base}${path}`, ca, {
              method,
              headers: { Origin: app },
            });
            expect(status, `${method} ${path}`).toBe(404);
          }
        }
      }
    };
    await expectNoOwnerSetupRoutes();

    let streams = 0;
    let snapshots = 0;
    page.on("request", (request) => {
      if (request.url() === `${base}/setup/node/events`) streams += 1;
      if (request.url() === `${base}/setup/node`) snapshots += 1;
    });
    await page.getByLabel("Administrator username", { exact: true }).fill(username);
    await page.getByLabel("Administrator password", { exact: true }).fill(password);
    const [accepted] = await Promise.all([
      page.waitForResponse((res) => res.url() === endpoint && res.request().method() === "POST"),
      page.getByRole("button", { name: "Create", exact: true }).click(),
    ]);
    expect(accepted.status()).toBe(202);
    await expect(page).toHaveURL(`${app}/signin`, { timeout: 60_000 });
    expect(streams).toBe(1);
    expect(snapshots).toBeLessThanOrEqual(2);
    expect((await readdir(data)).sort()).toEqual([
      "node-admission",
      "unified-setup.lock",
      "unified.redb",
    ]);
    expect(await code(resolve(data, "node-admission/state"), true)).toBe(nodeCode);
    expect(await descriptor(page, base)).toEqual({
      state: "ready",
      idp,
      storage,
      login_client: {
        client_id: expect.stringMatching(/\S/),
        scopes: expect.arrayContaining(["openid"]),
      },
    });
    const ready = await descriptor(page, base);
    const signIn = async (name: string, credential: string) => {
      await page.getByLabel("Username", { exact: true }).fill(name);
      await page.getByLabel("Password", { exact: true }).fill(credential);
      const response = page.waitForResponse(
        (res) =>
          new URL(res.url()).pathname === "/idp/oauth2/token" && res.request().method() === "POST"
      );
      await page.getByRole("button", { name: "Sign in", exact: true }).click();
      const token = await response;
      expect(new URLSearchParams(token.request().postData() ?? "").get("client_id")).toBe(
        ready.login_client.client_id
      );
      return token.ok() ? ((await token.json()).access_token as string) : null;
    };
    expect(await signIn("admin@local.test", "password")).toBeNull();
    expect(await signIn(username, password)).toEqual(expect.any(String));
    await expect(page).not.toHaveURL(`${app}/signin`);
    await page.goto(`${app}/clients/${ready.login_client.client_id}`);
    await expect(page.getByRole("textbox", { name: "Name", exact: true })).toHaveValue(
      "Local Node operator"
    );
    await node.restart();
    await expect
      .poll(() => descriptor(page, base).catch(() => undefined), {
        timeout: 60_000,
      })
      .toEqual(ready);
    await expectNoOwnerSetupRoutes();
    await page.goto(app);
    await expect(page.getByRole("heading", { name: "Dashboard", exact: true })).toBeVisible();
    await page.goto(`${app}/signin`);
    const operatorToken = await signIn(username, password);
    expect(operatorToken).toEqual(expect.any(String));
    if (!operatorToken) throw new Error("Missing operator token");
    expect(await status(`${idp}/.well-known/openid-configuration`, ca)).toBe(200);
    for (const endpoint of [
      `${base}/setup/node`,
      `${idp}/.well-known/openid-configuration`,
      `${storage}/health`,
    ]) {
      expect((await api(endpoint, ca, { headers: { Origin: pm } })).origin).toBe(pm);
      expect(
        (
          await api(endpoint, ca, {
            headers: { Origin: "https://other.localhost" },
          })
        ).origin
      ).toBeUndefined();
    }
    expect(await descriptor(pmPage, base)).toEqual(ready);
    const pmSession = await passwordManagerSignin(
      pmPage,
      { origin: pm, idp, storage, username, password },
      true
    );
    const owner = installationOwner(base, ca);
    const operator = {
      clientId: ready.login_client.client_id as string,
      accessToken: operatorToken,
    };
    const read = await owner.exchange(pmSession, "storage:databases", "read");
    const databases = () => owner.json<{ id: string }[]>(`${storage}/databases`, read);
    const user = await owner.json<{ sub: string }>(`${idp}/userinfo`, pmSession.accessToken);

    await pmPage.getByRole("button", { name: "Lock vault" }).click();
    const vault = pmPage.getByRole("button", { name: new RegExp(localVault) });
    await expect(vault).toContainText(`ID: ${vaultId}`);
    const connect = pmPage.getByRole("button", { name: "Connect local database" });
    const destination = pmPage.getByRole("region", { name: "Confirm sync destination" });
    const confirm = destination.getByRole("button", { name: "Confirm and upload" });
    const retry = destination.getByRole("button", { name: "Retry first upload" });
    await connect.click();
    await expect(destination).toContainText(idp);
    await expect(destination).toContainText(user.sub);
    await expect(destination).toContainText(pm);
    await expect(destination).toContainText(storage);
    const lookup = `${storage}/databases/by-creation-key?creation_key=password-manager-local-v1`;
    expect((await owner.call(lookup, read, "GET")).status).toBe(404);
    expect(await databases()).toEqual([]);
    await confirm.click();
    const selectionError = destination.getByRole("alert");
    await expect(selectionError).toContainText("Only that owner can set Selection");
    await expect(selectionError).toContainText("read access to this Resource");
    await expect(selectionError).toContainText("devices.select permission for this Application");
    const catalog = await databases();
    expect(catalog).toHaveLength(1);
    const resourceId = catalog[0]?.id;
    if (!resourceId) throw new Error("Missing ensured Resource");
    await expect(destination).toContainText(resourceId);
    const resource = `${storage}/databases/${resourceId}`;
    const query = (...statements: string[]) =>
      owner.call(`${resource}/sql`, read, "POST", {
        statements: statements.map((sql) => ({ sql, params: [] })),
      });
    expect((await query("SELECT id FROM vaults")).status).toBe(400);

    // Only this fresh fixture has exactly one approved Device: the serving Node.
    const device = `${base}/device`;
    const operatorDevice = await owner.exchange(operator, device);
    const devices = await owner.json<{ id: string; state: string }[]>(
      `${device}/devices`,
      operatorDevice
    );
    expect(devices).toEqual([
      expect.objectContaining({ id: expect.any(String), state: "approved" }),
    ]);
    const deviceId = devices[0]?.id;
    if (!deviceId) throw new Error("Missing serving Device");
    const { applicationId } = await owner.json<{ applicationId: string }>(resource, read);
    const management = await owner.exchange(operator, "idp-management");
    const policy = `${base}/management/applications/${applicationId}`;
    const role = await owner.json<{ id: string }>(`${policy}/roles`, management, "POST", {
      name: "Select vault Resource",
    });
    const permission = await owner.json<{ id: string }>(
      `${policy}/permissions`,
      management,
      "POST",
      { name: "devices.select" }
    );
    for (const assignment of [
      `${policy}/roles/${role.id}/permissions/${permission.id}`,
      `${policy}/users/${user.sub}/roles/${role.id}`,
    ]) {
      const response = await owner.call(assignment, management, "POST");
      expect(response.status, response.body).toBeLessThan(300);
    }
    const pmDevice = await owner.exchange(pmSession, device);
    const select = () =>
      owner.call(`${device}/devices/${deviceId}/selection`, pmDevice, "PUT", {
        applicationId,
        id: resourceId,
        kind: "database",
        storageAccessToken: read,
      });
    const selected = await select();
    expect(selected.status, selected.body).toBe(204);

    await rejectNextBindingWrite(pmPage);
    await retry.click();
    const retrySave = pmPage.getByRole("button", { name: "Retry local save" });
    await expect(retrySave).toBeVisible();
    await pmPage.reload();
    await expect(retrySave).toBeVisible();
    const blockedRequests: string[] = [];
    const backendRoutes = `${base}/**`;
    await pmPage.route(backendRoutes, (route) => {
      blockedRequests.push(route.request().url());
      return route.abort();
    });
    const blockedSockets: string[] = [];
    pmPage.on("websocket", (socket) => blockedSockets.push(socket.url()));
    const receiptKey = "password-manager-v1:storage-binding:receipt";
    const receipt = await pmPage.evaluate((key) => localStorage.getItem(key), receiptKey);
    if (!receipt) throw new Error("Missing pending upload receipt");
    expect(JSON.parse(receipt)).toMatchObject({
      version: 1,
      loginState: expect.any(String),
      id: resourceId,
      confirmationId: expect.any(String),
      expected: null,
    });
    await pmPage.evaluate(async (key) => {
      const fixture = "/src/test/binding-browser-fixture.ts";
      const { namespace } = JSON.parse(localStorage.getItem(key) ?? "null");
      await (await import(fixture)).writeCompetingBinding(namespace);
    }, receiptKey);
    await retrySave.click();
    await expect(pmPage.getByRole("alert")).toContainText("a competing local binding was kept");
    await expect(pmPage.getByText("Linked Storage Resource ID: competing-resource")).toBeVisible();
    expect(await pmPage.evaluate((key) => localStorage.getItem(key), receiptKey)).toBe(receipt);
    // Clear only the fixture binding to restore the empty revision needed by the receipt.
    await pmPage.evaluate(async () => {
      const fixture = "/src/test/binding-browser-fixture.ts";
      await (await import(fixture)).removeCompetingBinding();
    });
    await pmPage.unroute(backendRoutes);
    await pmPage.reload();
    await expect(retrySave).toBeVisible();
    blockedRequests.length = 0;
    await pmPage.route(backendRoutes, (route) => {
      blockedRequests.push(route.request().url());
      return route.abort();
    });
    await retrySave.click();
    await expect(
      pmPage.getByText(`Linked Storage Resource ID: ${resourceId}`, { exact: true })
    ).toBeVisible();
    expect(blockedSockets.filter((url) => url.startsWith(base.replace(/^http/, "ws")))).toEqual([]);
    expect(blockedRequests).toEqual([]);
    expect(await pmPage.evaluate((key) => localStorage.getItem(key), receiptKey)).toBeNull();
    await pmPage.unroute(backendRoutes);
    await expect(pmPage.getByRole("button", { name: "Sync now", exact: true })).toHaveCount(0);
    await pmPage.getByRole("button", { name: "Reattach sync" }).click();
    expect(await databases()).toEqual(catalog);
    const remote = () =>
      query(
        "SELECT id, authentication FROM vaults",
        "SELECT vault_id, ciphertext, nonce FROM secrets"
      );
    const saved = await remote();
    expect(saved.status, saved.body).toBe(200);
    const { results } = JSON.parse(saved.body);
    expect(results[0].rows).toEqual([
      [
        { type: "text", value: vaultId },
        { type: "text", value: expect.any(String) },
      ],
    ]);
    expect(JSON.parse(results[0].rows[0][1].value)).toEqual({
      version: 1,
      ciphertext: expect.stringMatching(/^[A-Za-z0-9+/]+=*$/),
      nonce: expect.stringMatching(/^[A-Za-z0-9+/]{16}$/),
    });
    expect(results[1].rows).toEqual([
      [
        { type: "text", value: vaultId },
        { type: "text", value: expect.stringMatching(/^[A-Za-z0-9+/]+=*$/) },
        { type: "text", value: expect.stringMatching(/^[A-Za-z0-9+/]{16}$/) },
      ],
    ]);
    for (const plaintext of [localRecord, localSecret, masterPassword])
      expect(saved.body).not.toContain(plaintext);

    await node.restart();
    await expect.poll(() => descriptor(page, base).catch(() => undefined)).toEqual(ready);
    await pmPage.reload();
    const linked = pmPage.getByText(`Linked Storage Resource ID: ${resourceId}`, { exact: true });
    await expect(linked).toBeVisible();
    await expect(connect).toHaveCount(0);
    // Expire the client's cached token without changing the real IdP clock.
    await pmPage.clock.setFixedTime(Date.now() + 30 * 24 * 60 * 60 * 1000);
    const renewed = pmPage.waitForResponse(
      (response) =>
        response.url() === `${idp}/oauth2/token` &&
        new URLSearchParams(response.request().postData() ?? "").get("grant_type") ===
          "refresh_token"
    );
    await pmPage.getByRole("button", { name: "Sync now", exact: true }).click();
    expect((await renewed).ok()).toBe(true);
    await expect(pmPage.getByText("Sync complete.", { exact: true })).toBeVisible();
    await pmPage.reload();
    await expect(pmPage.getByRole("button", { name: "Sync now", exact: true })).toBeVisible();
    expect(await databases()).toEqual(catalog);
    expect(JSON.parse((await remote()).body)).toEqual(JSON.parse(saved.body));

    const findRemote = (page: Page) => page.getByRole("button", { name: "Find remote database" });
    const confirmRestore = (page: Page) =>
      page.getByRole("button", { name: "Restore database", exact: true });
    const writeCatalog = await owner.exchange(pmSession, "storage:databases", "write");
    const sameName = await owner.call(`${storage}/databases`, writeCatalog, "POST", {
      name: "Password Manager local database",
    });
    expect(sameName.status, sameName.body).toBe(201);
    expect(await owner.json<{ id: string }>(lookup, read)).toMatchObject({ id: resourceId });
    const catalogWithDuplicateName = await databases();
    expect(catalogWithDuplicateName).toHaveLength(2);
    await expect(findRemote(pmPage)).toHaveCount(0);
    await expect(vault).toContainText(`ID: ${vaultId}`);

    const restoredContext = await browser.newContext();
    const restoredPage = await restoredContext.newPage();
    const restoredSession = await passwordManagerSignin(
      restoredPage,
      { origin: pm, idp, storage, username, password },
      false,
      false
    );
    await findRemote(restoredPage).click();
    await expect(
      restoredPage.getByRole("region", { name: "Confirm remote database restore" })
    ).toContainText(resourceId);
    await confirmRestore(restoredPage).click();
    const restoredVault = restoredPage.getByRole("button", { name: new RegExp(localVault) });
    await expect(restoredVault).toContainText(`ID: ${vaultId}`);
    const binding = restoredPage.getByText(`Linked Storage Resource ID: ${resourceId}`, {
      exact: true,
    });
    await expect(binding).toBeVisible();
    await restoredPage.reload();
    await expect(binding).toBeVisible();
    await expect(restoredVault).toHaveCount(1);
    const offline = `${base}/**`;
    await restoredPage.route(offline, (route) => route.abort());
    await restoredPage.reload();
    await restoredVault.click();
    await restoredPage.getByLabel("Master password", { exact: true }).fill(masterPassword);
    await restoredPage.getByRole("button", { name: "Unlock", exact: true }).click();
    await restoredPage.getByRole("button", { name: localRecord, exact: true }).click();
    await restoredPage
      .getByRole("region", { name: "Item details" })
      .getByRole("button", { name: "Edit", exact: true })
      .click();
    await expect(restoredPage.getByLabel("Notes", { exact: true })).toHaveValue(localSecret);
    await restoredPage
      .getByRole("textbox", { name: "Name", exact: true })
      .fill("Edited restored vault offline");
    await restoredPage.getByRole("button", { name: "Save changes" }).click();
    await expect(
      restoredPage.getByRole("button", { name: "Edited restored vault offline", exact: true })
    ).toBeVisible();
    expect(await databases()).toEqual(catalogWithDuplicateName);
    expect(JSON.parse((await remote()).body)).toEqual(JSON.parse(saved.body));

    await restoredPage.getByRole("button", { name: "Lock vault" }).click();
    await restoredPage.unroute(offline);
    await restoredPage.reload();
    await expect(binding).toBeVisible();
    const deselected = await owner.call(
      `${device}/devices/${deviceId}/selection/${applicationId}/database/${resourceId}`,
      pmDevice,
      "DELETE"
    );
    expect(deselected.status, deselected.body).toBe(204);
    const sync = restoredPage.getByRole("button", { name: "Sync now", exact: true });
    await sync.click();
    await expect(restoredPage.getByRole("alert")).toContainText("Your changes stay local");
    expect(JSON.parse((await remote()).body)).toEqual(JSON.parse(saved.body));

    // Keep the exchange on a real server but hold client messages to test cancellation.
    const socketUrl = `${base.replace(/^http/, "ws")}/storage/sync/ws`;
    const holds: { started: boolean; closed: boolean }[] = [];
    function holdNextExchange() {
      const hold = { started: false, closed: false };
      holds.push(hold);
      return hold;
    }
    await restoredPage.routeWebSocket(socketUrl, (socket) => {
      const server = socket.connectToServer();
      const hold = holds.shift();
      if (hold) {
        socket.onMessage(() => {
          hold.started = true;
        });
        socket.onClose(() => {
          hold.closed = true;
          void server.close();
        });
      }
    });
    expect((await select()).status).toBe(204);
    const detachedExchange = holdNextExchange();
    await sync.click();
    await expect.poll(() => detachedExchange.started).toBe(true);
    await restoredPage.getByRole("button", { name: "Detach sync" }).click();
    await expect.poll(() => detachedExchange.closed).toBe(true);
    await expect(restoredPage.getByRole("button", { name: "Reattach sync" })).toBeVisible();
    await restoredPage.reload();
    await expect(binding).toBeVisible();
    await expect(sync).toHaveCount(0);
    expect(JSON.parse((await remote()).body)).toEqual(JSON.parse(saved.body));
    await restoredPage.getByRole("button", { name: "Reattach sync" }).click();
    await sync.click();
    await expect(restoredPage.getByText("Sync complete.", { exact: true })).toBeVisible();
    const changedRemote = await remote();
    expect(changedRemote.status, changedRemote.body).toBe(200);
    expect(JSON.parse(changedRemote.body)).not.toEqual(JSON.parse(saved.body));
    expect(changedRemote.body).not.toContain("Edited restored vault offline");
    expect(await databases()).toEqual(catalogWithDuplicateName);

    // A sign-out in one tab must cancel its exchange and lock another tab's open vault.
    const unlocked = await restoredContext.newPage();
    await unlocked.goto(pm);
    await unlocked.getByRole("button", { name: new RegExp(localVault) }).click();
    await unlocked.getByLabel("Master password", { exact: true }).fill(masterPassword);
    await unlocked.getByRole("button", { name: "Unlock", exact: true }).click();
    await expect(
      unlocked.getByRole("button", { name: "Edited restored vault offline" })
    ).toBeVisible();
    // Delay the native removal notification until the replacement token is already visible.
    const tokenKey = await restoredPage.evaluate((token) => {
      const key = Object.keys(localStorage).find((entry) =>
        localStorage.getItem(entry)?.includes(token)
      );
      if (!key) throw new Error("Missing signed-in session");
      return key;
    }, restoredSession.accessToken);
    await unlocked.evaluate((key) => {
      const captured = new Promise<StorageEvent>((resolve) => {
        function hold(event: StorageEvent) {
          if (event.key !== key || event.newValue !== null) return;
          event.stopImmediatePropagation();
          window.removeEventListener("storage", hold, true);
          resolve(event);
        }
        window.addEventListener("storage", hold, true);
      });
      window.addEventListener(
        "release-token-removal",
        () => {
          void captured.then((event) => window.dispatchEvent(event));
        },
        { once: true }
      );
    }, tokenKey);
    await restoredPage.evaluate((key) => {
      const stored = localStorage.getItem(key);
      if (!stored) throw new Error("Missing signed-in session");
      localStorage.removeItem(key);
      localStorage.setItem(key, stored);
    }, tokenKey);
    await unlocked.evaluate(() => window.dispatchEvent(new Event("release-token-removal")));
    await expect(unlocked).toHaveURL(`${pm}/`);
    await expect(unlocked.getByRole("link", { name: "Sign in to sync" })).toBeVisible();
    await unlocked.reload();
    await unlocked.getByRole("button", { name: new RegExp(localVault) }).click();
    await unlocked.getByLabel("Master password", { exact: true }).fill(masterPassword);
    await unlocked.getByRole("button", { name: "Unlock", exact: true }).click();
    await expect(
      unlocked.getByRole("button", { name: "Edited restored vault offline" })
    ).toBeVisible();
    const signedOutExchange = holdNextExchange();
    await sync.click();
    await expect.poll(() => signedOutExchange.started).toBe(true);
    const attachmentKey = `password-manager-v1:storage-binding:attached:${JSON.parse(receipt).namespace}`;
    const attachment = await restoredPage.evaluate(
      (key) => localStorage.getItem(key),
      attachmentKey
    );
    expect(attachment).not.toBeNull();
    await rejectStorageChanges(restoredPage, [attachmentKey]);
    await restoredPage.evaluate(() => {
      IDBFactory.prototype.open = () => {
        throw new DOMException("Injected database failure", "InvalidStateError");
      };
    });
    await restoredPage.getByRole("button", { name: "Sign out", exact: true }).click();
    await expect.poll(() => signedOutExchange.closed).toBe(true);
    expect(await restoredPage.evaluate((key) => localStorage.getItem(key), tokenKey)).toBeNull();
    expect(await restoredPage.evaluate((key) => localStorage.getItem(key), attachmentKey)).toBe(
      attachment
    );
    await restoredPage.reload();
    await expect(restoredPage.getByRole("link", { name: "Sign in to sync" })).toBeVisible();
    await expect(unlocked).toHaveURL(`${pm}/`);
    await expect(unlocked.getByRole("button", { name: new RegExp(localVault) })).toBeVisible();
    expect(JSON.parse((await remote()).body)).toEqual(JSON.parse(changedRemote.body));

    await restoredPage.getByRole("link", { name: "Sign in to sync" }).click();
    await restoredPage.getByRole("button", { name: "Sign in with IdP" }).click();
    await expect(restoredPage).toHaveURL(`${pm}/`);
    await expect(binding).toBeVisible();
    await expect(restoredPage.getByRole("button", { name: "Reattach sync" })).toBeVisible();
    await expect(sync).toHaveCount(0);
    await restoredPage.getByRole("button", { name: "Reattach sync" }).click();
    await sync.click();
    await expect(restoredPage.getByText("Sync complete.", { exact: true })).toBeVisible();
    // Old permission formats also keep the destination but require explicit reattachment.
    await restoredPage.evaluate((key) => {
      const permission = JSON.parse(localStorage.getItem(key) ?? "null");
      localStorage.setItem(key, permission[2]);
    }, attachmentKey);
    await restoredPage.reload();
    await expect(binding).toBeVisible();
    await expect(sync).toHaveCount(0);
    await restoredPage.getByRole("button", { name: "Reattach sync" }).click();
    await expect(sync).toBeVisible();

    // Failed credential cleanup stops sync here, but cannot make sign-out durable.
    await rejectStorageChanges(restoredPage, "all");
    await restoredPage.getByRole("button", { name: "Sign out", exact: true }).click();
    await expect(restoredPage.getByRole("alert")).toContainText("Sign-out is incomplete");
    await expect(restoredPage.getByRole("button", { name: "Retry sign-out" })).toBeVisible();
    await expect(restoredPage.getByRole("link", { name: "Sign in to sync" })).toHaveCount(0);
    await expect(sync).toHaveCount(0);
    expect(
      await restoredPage.evaluate((key) => localStorage.getItem(key), tokenKey)
    ).not.toBeNull();
    const reloadSockets: string[] = [];
    restoredPage.on("websocket", (socket) => reloadSockets.push(socket.url()));
    await restoredPage.reload();
    await expect(sync).toBeVisible();
    expect(reloadSockets).not.toContain(socketUrl);
    expect(JSON.parse((await remote()).body)).toEqual(JSON.parse(changedRemote.body));

    // Sign-out must finish while another tab still holds the binding lock.
    await unlocked.evaluate(
      () =>
        new Promise<void>((acquired) => {
          void navigator.locks.request(
            "password-manager-v1:storage-binding",
            () =>
              new Promise<void>((release) => {
                window.addEventListener("release-binding-lock", () => release(), { once: true });
                acquired();
              })
          );
        })
    );
    await restoredPage.getByRole("button", { name: "Sign out", exact: true }).click();
    await expect(restoredPage.getByRole("link", { name: "Sign in to sync" })).toBeVisible();
    expect(await restoredPage.evaluate((key) => localStorage.getItem(key), tokenKey)).toBeNull();
    await unlocked.evaluate(() => window.dispatchEvent(new Event("release-binding-lock")));

    await restoredPage.getByRole("link", { name: "Sign in to sync" }).click();
    await restoredPage.getByRole("button", { name: "Sign in with IdP" }).click();
    await expect(binding).toBeVisible();
    await expect(restoredPage.getByRole("button", { name: "Reattach sync" })).toBeVisible();
    await expect(sync).toHaveCount(0);
    await restoredPage.getByRole("button", { name: "Sign out", exact: true }).click();
    await expect(restoredPage.getByRole("link", { name: "Sign in to sync" })).toBeVisible();

    // Recreate the pending-save state with the real receipt from the original login.
    await pmPage.clock.setFixedTime(Date.now());
    await pmPage.evaluate(
      async ({ receiptKey, receipt }) => {
        const fixture = "/src/test/binding-browser-fixture.ts";
        await (await import(fixture)).removeCompetingBinding();
        const unversioned = JSON.parse(receipt);
        unversioned.version = undefined;
        localStorage.setItem(receiptKey, JSON.stringify(unversioned));
      },
      { receiptKey, receipt }
    );
    await pmPage.reload();
    await expect(connect).toBeVisible();
    await expect(retrySave).toHaveCount(0);
    await pmPage.evaluate(({ receiptKey, receipt }) => localStorage.setItem(receiptKey, receipt), {
      receiptKey,
      receipt,
    });
    await pmPage.reload();
    await expect(retrySave).toBeVisible();
    await rejectStorageChanges(pmPage, [receiptKey]);
    await pmPage.getByRole("button", { name: "Sign out", exact: true }).click();
    await expect(pmPage.getByRole("link", { name: "Sign in to sync" })).toBeVisible();
    expect(await pmPage.evaluate((key) => localStorage.getItem(key), receiptKey)).toBe(receipt);
    // The IdP session is still for the same User; this is a fresh Password Manager login.
    await pmPage.getByRole("link", { name: "Sign in to sync" }).click();
    await pmPage.getByRole("button", { name: "Sign in with IdP" }).click();
    await expect(connect).toBeVisible();
    await expect(retrySave).toHaveCount(0);
    await pmPage.reload();
    await expect(connect).toBeVisible();
    await expect(retrySave).toHaveCount(0);
    expect(await pmPage.evaluate((key) => localStorage.getItem(key), receiptKey)).toBe(receipt);
    expect(JSON.parse((await remote()).body)).toEqual(JSON.parse(changedRemote.body));

    const secondUser = "second@setup.test";
    const idpAdmin = await owner.exchange(operator, idp);
    await owner.json(`${idp}/users`, idpAdmin, "POST", {
      name: "Second User",
      email: secondUser,
      password,
    });
    await unlocked.goto(app);
    await unlocked.getByRole("button", { name: "Sign out", exact: true }).click();
    await expect(unlocked).toHaveURL(`${app}/signin`);
    const storageWrites: string[] = [];
    restoredPage.on("request", (request) => {
      if (request.url().startsWith(`${storage}/`) && !["GET", "OPTIONS"].includes(request.method()))
        storageWrites.push(request.method());
    });
    restoredPage.on("websocket", (socket) => {
      if (socket.url() === socketUrl) storageWrites.push("sync");
    });
    const different = await passwordManagerSignin(
      restoredPage,
      { origin: pm, idp, storage, username: secondUser, password },
      false,
      false,
      restoredSession.clientId
    );
    await expect(
      restoredPage.getByText("This database is linked to another account", { exact: false })
    ).toBeVisible();
    await expect(sync).toHaveCount(0);
    await expect(binding).toHaveCount(0);
    await expect(restoredVault).toContainText(`ID: ${vaultId}`);
    const differentUser = await owner.json<{ sub: string }>(
      `${idp}/userinfo`,
      different.accessToken
    );
    expect(differentUser.sub).not.toBe(user.sub);
    const differentRead = await owner.exchange(different, "storage:databases", "read");
    expect((await owner.call(`${storage}/databases`, differentRead)).status).toBe(401);
    await restoredVault.click();
    await restoredPage.getByLabel("Master password", { exact: true }).fill(masterPassword);
    await restoredPage.getByRole("button", { name: "Unlock", exact: true }).click();
    await expect(
      restoredPage.getByRole("button", { name: "Edited restored vault offline" })
    ).toBeVisible();
    expect(storageWrites).toEqual([]);
    expect(await databases()).toEqual(catalogWithDuplicateName);
    expect(JSON.parse((await remote()).body)).toEqual(JSON.parse(changedRemote.body));

    await pmPage.route(`${base}/**`, (route) => route.abort());
    await pmPage.reload();
    await expect(vault).toContainText(`ID: ${vaultId}`);
    await vault.click();
    await pmPage.getByLabel("Master password", { exact: true }).fill(masterPassword);
    await pmPage.getByRole("button", { name: "Unlock", exact: true }).click();
    await pmPage.getByRole("button", { name: localRecord, exact: true }).click();
    await pmPage
      .getByRole("region", { name: "Item details" })
      .getByRole("button", { name: "Edit", exact: true })
      .click();
    await expect(pmPage.getByLabel("Notes", { exact: true })).toHaveValue(localSecret);
    const changed = "Edited without Storage";
    await pmPage.getByRole("textbox", { name: "Name", exact: true }).fill(changed);
    await pmPage.getByRole("button", { name: "Save changes" }).click();
    await expect(pmPage.getByRole("button", { name: changed, exact: true })).toBeVisible();
  }
);

function api(
  url: string,
  ca: Buffer,
  options: {
    method?: string;
    headers?: Record<string, string>;
    body?: string;
  } = {}
): Promise<{ status: number; origin: string | undefined; body: string }> {
  return new Promise((resolve, reject) => {
    const req = request(
      url,
      { ca, timeout: 5000, method: options.method, headers: options.headers },
      (res) => {
        let body = "";
        res.setEncoding("utf8");
        res.on("data", (chunk: string) => {
          body += chunk;
        });
        res.on("end", () =>
          resolve({
            status: res.statusCode ?? 0,
            origin: res.headers["access-control-allow-origin"],
            body,
          })
        );
        res.on("error", reject);
      }
    );
    req.on("error", reject);
    req.on("timeout", () => req.destroy(new Error(`Timed out: ${url}`)));
    req.end(options.body);
  });
}

function installationOwner(base: string, ca: Buffer) {
  function call(url: string, token: string, method = "GET", body?: object) {
    return api(url, ca, {
      method,
      headers: {
        Authorization: `Bearer ${token}`,
        ...(body && { "Content-Type": "application/json" }),
      },
      ...(body && { body: JSON.stringify(body) }),
    });
  }
  async function json<T>(url: string, token: string, method?: string, body?: object): Promise<T> {
    const response = await call(url, token, method, body);
    expect(response.status, response.body).toBeLessThan(300);
    return JSON.parse(response.body);
  }
  async function exchange(
    session: { clientId: string; accessToken: string },
    resource: string,
    action?: "read" | "write"
  ) {
    const response = await api(`${base}/idp/oauth2/token`, ca, {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      body: new URLSearchParams({
        grant_type: "urn:ietf:params:oauth:grant-type:token-exchange",
        subject_token_type: "urn:ietf:params:oauth:token-type:access_token",
        subject_token: session.accessToken,
        client_id: session.clientId,
        resource,
        ...(action && {
          authorization_details: JSON.stringify([{ type: "storage", actions: [action] }]),
        }),
      }).toString(),
    });
    expect(response.status, `Token exchange for ${resource}`).toBe(200);
    return (JSON.parse(response.body) as { access_token: string }).access_token;
  }
  return { call, json, exchange };
}

async function remoteInstallation(node: Awaited<ReturnType<typeof launchNode>>) {
  const { url, data, app, ca, pm } = node;
  const remote = url("remote");
  const ui = url("remote-ui");
  const idp = `${remote}/idp`;
  const device = `${remote}/device`;
  const storage = `${remote}/storage`;
  const application = `${app}/join-fixture`;
  const username = "admin@local.test";
  const password = "password";
  node.launch(
    "pnpm",
    [
      "exec",
      "portless",
      "run",
      "--name",
      "remote",
      "cargo",
      "run",
      "--locked",
      "-p",
      "unified-server",
      "--",
      "seeded",
      "--data-dir",
      resolve(data, "../remote"),
      "--origin",
      ui,
      "--password-manager-origin",
      pm,
      "--join-origin",
      app,
    ],
    { RUST_MIN_STACK: "67108864" }
  );
  node.launch(
    "pnpm",
    [
      "exec",
      "portless",
      "run",
      "--name",
      "remote-ui",
      "pnpm",
      "--filter",
      "@ofsdk/app",
      "exec",
      "vite",
      "dev",
    ],
    {
      PUBLIC_URL: ui,
      PUBLIC_IDP_BASE_URL: idp,
      PUBLIC_DEVICE_BASE_URL: device,
      PUBLIC_STORAGE_BASE_URL: storage,
      PUBLIC_MANAGEMENT_BASE_URL: `${remote}/management`,
      PUBLIC_OIDC_CLIENT_ID: "local-login",
      PUBLIC_OIDC_APPLICATION_URI: `${ui}/local-login`,
    }
  );
  await node.ready(`${idp}/health`);
  await node.ready(ui);
  async function post(path: string, body: object | URLSearchParams, bearer?: string) {
    const response = await api(path, ca, {
      method: "POST",
      headers: {
        "Content-Type":
          body instanceof URLSearchParams
            ? "application/x-www-form-urlencoded"
            : "application/json",
        ...(bearer && { Authorization: `Bearer ${bearer}` }),
      },
      body: body instanceof URLSearchParams ? body.toString() : JSON.stringify(body),
    });
    expect(response.status, response.status >= 400 ? response.body : undefined).toBeLessThan(300);
    return JSON.parse(response.body);
  }
  const token = (params: Record<string, string>) =>
    post(`${idp}/oauth2/token`, new URLSearchParams(params));
  const session = await token({
    grant_type: "password",
    client_id: "local-login",
    username,
    password,
    scope: "openid profile email",
  });
  const exchange = async (resource: string) =>
    (
      await token({
        grant_type: "urn:ietf:params:oauth:grant-type:token-exchange",
        subject_token_type: "urn:ietf:params:oauth:token-type:access_token",
        subject_token: session.access_token,
        resource,
      })
    ).access_token as string;
  const registration = await post(
    `${idp}/oauth2/register`,
    {
      application: { uri: application, name: "Join fixture" },
      client_name: "Join fixture",
      profile: "web_application",
      token_endpoint_auth_method: "none",
      client_type: "public",
      redirect_uris: [`${app}/setup/join`],
      response_types: ["code"],
      allowed_scopes: ["openid", "profile", "email"],
      allowed_grant_types: [
        "authorization_code",
        "refresh_token",
        "urn:ietf:params:oauth:grant-type:token-exchange",
      ],
    },
    await exchange(idp)
  );
  const deviceToken = await exchange(device);
  const key = generateKeyPairSync("ed25519").publicKey.export({
    format: "jwk",
  });
  const publicKey = Buffer.from(key.x ?? "", "base64url").toString("hex");
  await post(
    `${device}/devices/enrollments`,
    { name: "Accepting Device", publicKey, address: "127.0.0.1:23456" },
    deviceToken
  );
  const devices = async () => {
    const response = await api(`${device}/devices`, ca, {
      headers: { Authorization: `Bearer ${deviceToken}` },
    });
    expect(response.status).toBe(200);
    return JSON.parse(response.body) as {
      id: string;
      name: string;
      publicKey: string;
      state: string;
    }[];
  };
  return {
    idp,
    device,
    storage,
    application,
    client: registration.client_id as string,
    publicKey,
    username,
    password,
    devices,
  };
}

nodeTest("Join refuses Storage outside the selected Installation", async ({ node }) => {
  const { app, base, browser, ca } = node;
  const remote = await remoteInstallation(node);
  const page = await browser.newPage();
  const nodeCode = await choose(page, node, "join");
  const endpoint = `${base}/setup/join`;
  const post = (body: object) =>
    api(endpoint, ca, {
      method: "POST",
      headers: {
        Origin: app,
        "Content-Type": "application/json",
        Authorization: `Setup ${nodeCode}`,
      },
      body: JSON.stringify(body),
    });
  const before = await remote.devices();
  expect(
    (
      await post({
        action: "prepare",
        intent: {
          issuer: remote.idp,
          device: remote.device,
          storage: `${base}/storage`,
          clientId: remote.client,
          application: remote.application,
          acceptingPublicKey: remote.publicKey,
          name: "Wrong Installation",
          address: "127.0.0.1:23457",
        },
      })
    ).status
  ).toBe(200);
  const refused = await post({
    action: "request",
    accessToken: "must-not-be-forwarded",
  });
  expect(refused.status).toBe(409);
  expect(await descriptor(page, base)).toEqual({ state: "joining" });
  expect(await remote.devices()).toEqual(before);
});

nodeTest(
  "default Join resumes pending Device approval and retains it without a local authority",
  async ({ node }) => {
    const { app, base, browser, ca, data, pm } = node;
    const remote = await remoteInstallation(node);
    const page = await browser.newPage();
    const pmPage = await browser.newPage();
    await passwordManagerWhileWaiting(pmPage, pm, base, app);
    const nodeCode = await choose(page, node, "join");
    await expect(page.getByLabel("Local join helper")).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Prepare join" })).toBeVisible();
    const nodeCodeField = page.getByLabel("Node setup code", { exact: true });
    await expect(nodeCodeField).toHaveCount(0);
    expect(await descriptor(page, base)).toEqual({ state: "joining" });
    const endpoint = `${base}/setup/join`;
    const unauthorized = await api(endpoint, ca, {
      method: "POST",
      headers: { Origin: app, "Content-Type": "application/json" },
      body: '{"action":"resume"}',
    });
    expect(unauthorized.status).toBe(401);

    const name = "Joined through unified";
    for (const [label, value] of [
      ["Installation issuer", remote.idp],
      ["Device API", remote.device],
      ["Storage API", remote.storage],
      ["Join OAuth client ID", remote.client],
      ["Join Application URI", remote.application],
      ["Accepting Device public key", remote.publicKey],
      ["Joining Device name", name],
      ["Joining Device address", "127.0.0.1:23457"],
    ] as const)
      await page.getByLabel(label, { exact: true }).fill(value);
    await page.getByRole("button", { name: "Prepare join" }).click();
    await page.getByRole("button", { name: "Sign in to Installation" }).click();
    await page.getByLabel("Username", { exact: true }).fill(remote.username);
    await page.getByLabel("Password", { exact: true }).fill(remote.password);
    await page.getByRole("button", { name: "Sign in", exact: true }).click();
    await page.getByRole("button", { name: "Allow", exact: true }).click();
    await expect(page.getByText("Signed in to the Installation.")).toBeVisible();
    expect((await remote.devices()).some((device) => device.name === name)).toBe(false);
    await expect(nodeCodeField).toHaveValue("");
    await nodeCodeField.fill(nodeCode);
    await page.getByRole("button", { name: "Request pairing" }).click();
    await expect(page.getByText("Pairing is pending.")).toBeVisible();
    const pending = (await remote.devices()).find((device) => device.name === name);
    expect(pending?.state).toBe("pending");
    if (!pending) throw new Error("Missing pending Device");
    expect(await descriptor(page, base)).toEqual({ state: "joining" });
    await node.restart();
    await page.goto(app);
    await expect(page).toHaveURL(`${app}/setup/join`);
    await expect(nodeCodeField).toHaveValue("");
    await nodeCodeField.fill(nodeCode);
    await page.getByRole("button", { name: "Resume join" }).click();
    await expect(page.getByText("Pairing is pending.")).toBeVisible();
    await page.getByRole("button", { name: "Approve this Device" }).click();
    await expect(page.getByText("This Node joined as a Device.")).toBeVisible();
    await node.restart();
    const fresh = await browser.newPage();
    await fresh.goto(app);
    await expect(fresh).toHaveURL(`${app}/setup/join`);
    await expect(fresh.getByText("This Node joined as a Device.")).toBeVisible();
    await expect(fresh.getByText(pending.id, { exact: true })).toBeVisible();
    await expect(fresh.getByText(pending.publicKey, { exact: true })).toBeVisible();
    expect(await descriptor(fresh, base)).toEqual({
      state: "ready",
      idp: remote.idp,
      storage: remote.storage,
    });
    const approved = (await remote.devices()).filter((device) => device.name === name);
    expect(approved).toHaveLength(1);
    expect(approved[0]).toMatchObject({
      id: pending.id,
      publicKey: pending.publicKey,
      state: "approved",
    });
    expect(await status(`${base}/idp/.well-known/openid-configuration`, ca)).toBe(404);
    for (const owner of owners) {
      await expect(stat(resolve(data, owner))).rejects.toMatchObject({
        code: "ENOENT",
      });
      await expect(stat(resolve(data, `${owner}.setup-host`))).rejects.toMatchObject({
        code: "ENOENT",
      });
    }
    const pmSession = await passwordManagerSignin(
      pmPage,
      {
        origin: pm,
        idp: remote.idp,
        storage: remote.storage,
        username: remote.username,
        password: remote.password,
      },
      true
    );
    await pmPage.getByRole("button", { name: "Lock vault" }).click();
    const owner = installationOwner(new URL(remote.storage).origin, node.ca);
    const write = await owner.exchange(pmSession, "storage:databases", "write");
    const legacy = await owner.json<{ id: string }>(
      `${remote.storage}/databases/ensure`,
      write,
      "POST",
      {
        creationKey: "password-manager-main",
        name: "password-manager-main",
      }
    );
    await passwordManagerOpensLegacyVault(pmPage, pm, legacy.id);
  }
);
