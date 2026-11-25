import { type ChildProcess, execFileSync, spawn } from "node:child_process";
import { generateKeyPairSync } from "node:crypto";
import { once } from "node:events";
import { mkdir, mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import { request } from "node:https";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath } from "node:url";
import { chromium, expect, type Page, test } from "@playwright/test";

import { passwordManagerSignin, passwordManagerWhileWaiting } from "./password-manager-setup";

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

async function code(path: string) {
  const value = (await readFile(path, "utf8")).split("\n")[1];
  if (!value || !/^[a-f0-9]{64}$/.test(value) || (await stat(path)).mode & 0o077) {
    throw new Error("Setup code not available through protected local management");
  }
  return value;
}

const ownerCode = (root: string, owner: string) =>
  code(resolve(root, `${owner}.setup-host/setup-admission/state`));

async function fillCodes(page: Page, root: string) {
  for (const owner of owners) {
    await page.getByLabel(`${owner} host setup code`).fill(await ownerCode(root, owner));
  }
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
      backend = launch("just", ["unified", "--data-dir", data]);
      await ready(`${base}/setup/node`);
    };
    await startBackend();
    launch("pnpm", ["apps:dev"]);
    await ready(app);
    await ready(pm);
    const browser = await chromium.launch({ env: { ...process.env, HOME: tls.home } });
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
    // The IdP identity socket must fit in the OS Unix socket path.
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
  expect(await descriptor(page, node.base)).toEqual({ state: "awaiting_choice" });
  expect(await select(page, node.base, choice)).toBe(401);
  for (const owner of owners) {
    await expect(stat(resolve(node.data, `${owner}.setup-host`))).rejects.toMatchObject({
      code: "ENOENT",
    });
  }
  const nodeCode = await code(resolve(node.data, "node-admission/state"));
  await page.getByLabel("Node setup code").fill(nodeCode);
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
}

nodeTest(
  "default Create resumes after interruption and keeps the new identity after restart",
  async ({ node }) => {
    const { app, base, browser, ca, data, pm } = node;
    const idp = `${base}/idp`;
    const storage = `${base}/storage`;
    const page = await browser.newPage();
    const pmPage = await browser.newPage();
    await passwordManagerWhileWaiting(pmPage, pm, base);
    await choose(page, node, "create");
    await expect(
      page.getByRole("heading", { name: "Create the first Installation" })
    ).toBeVisible();
    expect(await descriptor(page, base)).toEqual({ state: "creating" });
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
    const denied = await api(`${idp}/setup/owner/create`, ca, {
      method: "POST",
      headers: {
        Origin: pm,
        "Content-Type": "application/json",
        Authorization: `Setup ${await ownerCode(data, "idp")}`,
      },
      body: JSON.stringify({ ...intent, password }),
    });
    expect(denied.status).toBe(403);
    expect(denied.origin).toBeUndefined();
    expect(await submitIdp()).toBe(401);
    expect(await submitIdp(await ownerCode(data, "idp"))).toBe(202);
    await page.goto(`${app}/signin`);
    await expect(page).toHaveURL(`${app}/setup/installation`);
    await node.restart();
    await page.reload();
    expect(await descriptor(page, base)).toEqual({ state: "creating" });
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
    expect(await descriptor(page, base)).toEqual({ state: "ready", idp, storage });
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
    await node.restart();
    await waitForReady();
    await page.goto(app);
    await expect(page.getByRole("heading", { name: "Dashboard", exact: true })).toBeVisible();
    expect((await evidence()).facts).toEqual(ready.facts);
    await page.goto(`${app}/signin`);
    expect(await signIn(username, password)).toBe(true);
    expect(await status(`${idp}/.well-known/openid-configuration`, ca)).toBe(200);
    for (const endpoint of [
      `${base}/setup/node`,
      `${idp}/.well-known/openid-configuration`,
      `${storage}/health`,
    ]) {
      expect((await api(endpoint, ca, { headers: { Origin: pm } })).origin).toBe(pm);
      expect(
        (await api(endpoint, ca, { headers: { Origin: "https://other.localhost" } })).origin
      ).toBeUndefined();
    }
    expect(await descriptor(pmPage, base)).toEqual({ state: "ready", idp, storage });
    await passwordManagerSignin(pmPage, { origin: pm, idp, storage, username, password }, true);
  }
);

function api(
  url: string,
  ca: Buffer,
  options: { method?: string; headers?: Record<string, string>; body?: string } = {}
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

async function remoteInstallation(node: Awaited<ReturnType<typeof launchNode>>) {
  const { url, data, app, ca } = node;
  const remote = url("remote");
  const ui = url("remote-ui");
  const idp = `${remote}/idp`;
  const device = `${remote}/device`;
  const storage = `${remote}/storage`;
  const application = `${app}/join-fixture`;
  const username = "admin@local.test";
  const password = "password";
  node.launch("pnpm", [
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
    app,
  ]);
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
  const key = generateKeyPairSync("ed25519").publicKey.export({ format: "jwk" });
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

nodeTest(
  "default Join resumes pending Device approval and retains it without a local authority",
  async ({ node }) => {
    const { app, base, browser, ca, data, pm } = node;
    const remote = await remoteInstallation(node);
    const page = await browser.newPage();
    await choose(page, node, "join");
    await expect(page.getByLabel("Local join helper")).toHaveCount(0);
    expect(await descriptor(page, base)).toEqual({ state: "joining" });
    const endpoint = `${base}/setup/join`;
    const joinCode = await code(resolve(data, "join/admission/state"));
    const unauthorized = await api(endpoint, ca, {
      method: "POST",
      headers: { Origin: app, "Content-Type": "application/json" },
      body: '{"action":"resume"}',
    });
    expect(unauthorized.status).toBe(401);
    const joinCodeField = page.getByLabel("Local setup code");
    await joinCodeField.fill(joinCode);
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
    await joinCodeField.fill(joinCode);
    await page.getByRole("button", { name: "Request pairing" }).click();
    await expect(page.getByText("Pairing is pending.")).toBeVisible();
    const pending = (await remote.devices()).find((device) => device.name === name);
    expect(pending?.state).toBe("pending");
    if (!pending) throw new Error("Missing pending Device");
    expect(await descriptor(page, base)).toEqual({ state: "joining" });
    await node.restart();
    await page.goto(app);
    await expect(page).toHaveURL(`${app}/setup/join`);
    await joinCodeField.fill(joinCode);
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
      await expect(stat(resolve(data, owner))).rejects.toMatchObject({ code: "ENOENT" });
      await expect(stat(resolve(data, `${owner}.setup-host`))).rejects.toMatchObject({
        code: "ENOENT",
      });
    }
    const pmPage = await browser.newPage();
    await pmPage.goto(pm);
    await pmPage.getByRole("link", { name: "Sign in to sync" }).click();
    const [pmDescriptor] = await Promise.all([
      pmPage.waitForResponse(`${base}/setup/node`),
      pmPage.getByRole("button", { name: "Sign in with IdP" }).click(),
    ]);
    expect(await pmDescriptor.json()).toEqual({
      state: "ready",
      idp: remote.idp,
      storage: remote.storage,
    });
  }
);
