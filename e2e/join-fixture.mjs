import { execFileSync, spawn } from "node:child_process";
import { generateKeyPairSync } from "node:crypto";
import { once } from "node:events";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { request as httpRequest } from "node:http";
import { createServer, request as httpsRequest } from "node:https";
import { createServer as reserve } from "node:net";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { setTimeout } from "node:timers/promises";
import { fileURLToPath } from "node:url";

import { passwordManagerIngress } from "./password-manager-ingress.mjs";

const root = fileURLToPath(new URL("../", import.meta.url));
const helper = resolve(root, process.env.E2E_JOIN_HELPER_BIN ?? "target/debug/join-node");
const unified = resolve(root, process.env.E2E_BACKEND_BIN ?? "target/debug/unified-server");
const hostname = "127.0.0.1";
const children = [];
const servers = [];
let scratch;

function run(command, args) {
  execFileSync(command, args, { cwd: root, stdio: "inherit", timeout: 30_000 });
}

function start(command, args, env = {}, cwd = root) {
  const child = spawn(command, args, {
    cwd,
    env: { ...process.env, ...env },
    stdio: "inherit",
  });
  const exit = once(child, "exit");
  children.push({ child, exit });
  return exit;
}

async function ports(count) {
  const sockets = [];
  try {
    for (let i = 0; i < count; i++) {
      const socket = reserve().listen(0, hostname);
      sockets.push(socket);
      await once(socket, "listening");
    }
    return sockets.map((socket) => socket.address().port);
  } finally {
    for (const socket of sockets) socket.close();
  }
}

function call(url, ca, options = {}) {
  const { bearer, form, json, origin } = options;
  const body = form ? new URLSearchParams(form).toString() : json ? JSON.stringify(json) : null;
  const headers = {};
  if (origin) headers.Origin = origin;
  if (bearer) headers.Authorization = `Bearer ${bearer}`;
  if (body) {
    headers["content-type"] = form ? "application/x-www-form-urlencoded" : "application/json";
    headers["content-length"] = Buffer.byteLength(body);
  }
  return new Promise((resolve, reject) => {
    const req = httpsRequest(url, { ca, headers, method: body ? "POST" : "GET" }, (res) => {
      res.setEncoding("utf8");
      let text = "";
      res.on("data", (chunk) => {
        text += chunk;
      });
      res.on("end", () => {
        if (res.statusCode !== 200) reject(new Error(`${url}: HTTP ${res.statusCode}`));
        else resolve(text);
      });
      res.on("error", reject);
    });
    req.on("error", reject);
    req.setTimeout(5000, () => req.destroy(new Error("fixture owner timed out")));
    req.end(body);
  });
}

async function ready(url, ca) {
  for (let i = 0; i < 100; i++) {
    try {
      await call(url, ca);
      return;
    } catch {
      await setTimeout(200);
    }
  }
  throw new Error(`Fixture not ready: ${url}`);
}

async function main() {
  scratch = await mkdtemp(join(tmpdir(), "of-join-"));
  const [remotePort, appPort, helperPort, backendPort, vitePort, pmPort, pmVitePort] =
    await ports(7);
  const remote = `https://${hostname}:${remotePort}`;
  const app = `https://${hostname}:${appPort}`;
  const pm = `https://${hostname}:${pmPort}`;
  const issuer = `${remote}/idp`;
  const device = `${remote}/device`;
  const storage = `${remote}/storage`;
  const application = `${app}/join-fixture`;
  const joinRoot = join(scratch, "node");
  run(helper, ["init", joinRoot, String(helperPort), app]);
  const caPath = join(scratch, "ingress-ca.pem");
  const caKey = join(scratch, "ingress-ca.key");
  const leafPath = join(scratch, "ingress.pem");
  const leafKey = join(scratch, "ingress.key");
  const csrPath = join(scratch, "ingress.csr");
  const extension = join(scratch, "ingress.ext");
  await writeFile(extension, "subjectAltName=IP:127.0.0.1\nextendedKeyUsage=serverAuth\n");
  const ec = ["-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256", "-noenc"];
  run("openssl", [
    "req",
    "-new",
    "-x509",
    ...ec,
    "-days",
    "1",
    "-subj",
    "/CN=Join test ingress CA",
    "-keyout",
    caKey,
    "-out",
    caPath,
  ]);
  run("openssl", [
    "req",
    "-new",
    ...ec,
    "-subj",
    "/CN=127.0.0.1",
    "-keyout",
    leafKey,
    "-out",
    csrPath,
  ]);
  run("openssl", [
    "x509",
    "-req",
    "-in",
    csrPath,
    "-CA",
    caPath,
    "-CAkey",
    caKey,
    "-set_serial",
    "1",
    "-days",
    "1",
    "-extfile",
    extension,
    "-out",
    leafPath,
  ]);
  const ca = await readFile(caPath);
  const cert = `${await readFile(leafPath)}\n${ca}`;
  const key = await readFile(leafKey);
  const caBundle = join(scratch, "test-cas.pem");
  const trusted = `${ca}\n${await readFile(join(joinRoot, "tls/ca.pem"))}`;
  await writeFile(caBundle, trusted);
  const browserHome = join(scratch, "browser");
  const nss = `sql:${join(browserHome, ".pki/nssdb")}`;
  await mkdir(join(browserHome, ".pki/nssdb"), { recursive: true });
  run("certutil", ["-N", "-d", nss, "--empty-password"]);
  run("certutil", ["-A", "-d", nss, "-n", "Join fixture ingress", "-t", "C,,", "-i", caPath]);
  run("certutil", [
    "-A",
    "-d",
    nss,
    "-n",
    "Join fixture helper",
    "-t",
    "C,,",
    "-i",
    join(joinRoot, "tls/ca.pem"),
  ]);

  // Route to the real owners. Lose exactly one successful pairing response after the owner commits.
  let dropPairing = true;
  for (const port of [remotePort, appPort, pmPort]) {
    const server = createServer({ cert, key }, async (req, res) => {
      const isRemote = port === remotePort;
      if (
        port === appPort &&
        req.headers.origin === pm &&
        req.method === "GET" &&
        req.url === "/setup/node"
      ) {
        const state = await call(`https://${hostname}:${helperPort}/setup/join`, trusted, {
          origin: app,
        })
          .then((value) => JSON.parse(value).stage)
          .catch(() => undefined);
        if (
          passwordManagerIngress(
            req,
            res,
            pm,
            state === "approved"
              ? { state: "ready", idp: issuer, storage }
              : { state: "awaiting_choice" }
          )
        )
          return;
      }
      if (isRemote && passwordManagerIngress(req, res, pm)) return;
      const owner = isRemote && /^\/(idp|device|management|storage)\//.test(req.url);
      const upstream = httpRequest(
        {
          hostname,
          port: owner ? backendPort : port === pmPort ? pmVitePort : vitePort,
          path: req.url,
          method: req.method,
          headers: req.headers,
        },
        (response) => {
          if (
            dropPairing &&
            owner &&
            req.method === "POST" &&
            req.url === "/device/devices/pairings" &&
            response.statusCode === 200
          ) {
            dropPairing = false;
            response.resume();
            response.once("end", () => res.destroy());
            return;
          }
          res.writeHead(response.statusCode, { ...response.headers, ...res.getHeaders() });
          response.pipe(res);
        }
      );
      upstream.on("error", () => {
        res.writeHead(502);
        res.end();
      });
      req.pipe(upstream);
    });
    servers.push(server);
    server.listen(port, hostname);
    await once(server, "listening");
  }

  start(
    unified,
    [
      "seeded",
      "--data-dir",
      join(scratch, "installation"),
      "--origin",
      remote,
      "--password-manager-origin",
      app,
    ],
    {
      PORT: String(backendPort),
      PUBLIC_BASE_URL: remote,
      RUST_MIN_STACK: process.env.RUST_MIN_STACK ?? "67108864",
      SSL_CERT_FILE: caBundle,
    }
  );
  start(
    process.execPath,
    ["node_modules/vite/bin/vite.js", "dev", "--host", hostname, "--strictPort"],
    {
      PORT: String(vitePort),
      PUBLIC_URL: remote,
      PUBLIC_IDP_BASE_URL: issuer,
      PUBLIC_DEVICE_BASE_URL: device,
      PUBLIC_STORAGE_BASE_URL: storage,
      PUBLIC_MANAGEMENT_BASE_URL: `${remote}/management`,
      PUBLIC_OIDC_CLIENT_ID: "local-login",
      PUBLIC_OIDC_APPLICATION_URI: `${remote}/local-login`,
    },
    join(root, "apps/app")
  );
  start(
    process.execPath,
    ["node_modules/vite/bin/vite.js", "dev", "--host", hostname, "--strictPort"],
    { PORT: String(pmVitePort), PUBLIC_UNIFIED_ORIGIN: app },
    join(root, "apps/password-manager")
  );
  await ready(pm, ca);
  await ready(`${issuer}/health`, ca);
  const username = "admin@local.test";
  const password = "password";
  const request = async (url, options) => JSON.parse(await call(url, ca, options));
  const session = await request(`${issuer}/oauth2/token`, {
    form: {
      grant_type: "password",
      client_id: "local-login",
      username,
      password,
      scope: "openid profile email",
    },
  });
  const exchange = async (resource) =>
    (
      await request(`${issuer}/oauth2/token`, {
        form: {
          grant_type: "urn:ietf:params:oauth:grant-type:token-exchange",
          subject_token_type: "urn:ietf:params:oauth:token-type:access_token",
          subject_token: session.access_token,
          resource,
        },
      })
    ).access_token;
  const registered = await request(`${issuer}/oauth2/register`, {
    bearer: await exchange(issuer),
    json: {
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
  });
  const deviceToken = await exchange(device);
  const acceptingKey = Buffer.from(
    generateKeyPairSync("ed25519").publicKey.export({ format: "jwk" }).x,
    "base64url"
  ).toString("hex");
  await request(`${device}/devices/enrollments`, {
    bearer: deviceToken,
    json: {
      name: "Accepting Device",
      publicKey: acceptingKey,
      address: "127.0.0.1:23456",
    },
  });
  await ready(`${app}/setup/join`, ca);
  const code = (await readFile(join(joinRoot, "admission/state"), "utf8"))
    .split("\n")
    .find((line) => /^[0-9a-f]{64}$/.test(line));
  if (!code || !registered.client_id) throw new Error("Fixture is missing join credentials");
  const [status] = await start(
    process.execPath,
    ["node_modules/@playwright/test/cli.js", "test", "--config", "e2e/join.playwright.config.ts"],
    {
      NODE_EXTRA_CA_CERTS: caBundle,
      SSL_CERT_FILE: caBundle,
      JOIN_TEST_BROWSER_HOME: browserHome,
      JOIN_TEST_ROOT: joinRoot,
      JOIN_TEST_PORT: String(helperPort),
      JOIN_TEST_BIN: helper,
      JOIN_TEST_APP_ORIGIN: app,
      JOIN_TEST_PASSWORD_MANAGER: pm,
      JOIN_TEST_DEVICE: device,
      JOIN_TEST_ISSUER: issuer,
      JOIN_TEST_STORAGE: storage,
      JOIN_TEST_CLIENT: registered.client_id,
      JOIN_TEST_APPLICATION: application,
      JOIN_TEST_CODE: code,
      JOIN_TEST_DEVICE_TOKEN: deviceToken,
      JOIN_TEST_ACCEPTING_KEY: acceptingKey,
      JOIN_TEST_USERNAME: username,
      JOIN_TEST_PASSWORD: password,
      JOIN_TEST_LOSES_RESPONSE: "1",
    }
  );
  return status ?? 1;
}

try {
  process.exitCode = await main();
} finally {
  for (const { child } of children)
    if (child.exitCode === null && child.signalCode === null) child.kill("SIGTERM");
  for (const server of servers) {
    server.closeAllConnections();
    server.close();
  }
  await Promise.allSettled(children.map(({ exit }) => exit));
  if (scratch) await rm(scratch, { recursive: true, force: true });
}
