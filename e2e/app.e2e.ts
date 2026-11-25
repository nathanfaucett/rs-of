import { expect, type Page, test } from "@playwright/test";
import { appURL, backendURL } from "./backend.mjs";
import { json, signIn, storageClient } from "./session";

test("invalid service selection opens configuration", async ({ page }) => {
  await page.goto(`${appURL}/config`);
  await page.evaluate(() => {
    localStorage.setItem("idp-api-url", JSON.stringify("https://invalid.localhost:1355/idp"));
  });
  await page.goto(`${appURL}/`);
  await expect(page).toHaveURL(`${appURL}/config`);
  await expect(page.getByRole("heading", { name: "Configuration", exact: true })).toBeVisible();
});

test("configuration discovers and saves unified service URLs", async ({ page }) => {
  await page.goto(`${appURL}/config`);
  await page.getByRole("button", { name: "Detect services", exact: true }).click();
  await expect(
    page.getByText("Unified services detected. Review the four URLs and save.", {
      exact: true,
    })
  ).toBeVisible();
  for (const [label, owner] of [
    ["IdP URL", "idp"],
    ["Management URL", "management"],
    ["Device API URL", "device"],
    ["Storage API URL", "storage"],
  ] as const) {
    await expect(page.getByRole("textbox", { name: label, exact: true })).toHaveValue(
      `${backendURL}/${owner}`
    );
  }
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page).toHaveURL(/\/signin/);
});

test("browser sign-in bypasses installation setup and retains its protected session on reload", async ({
  page,
}) => {
  await page.goto(`${appURL}/devices`);
  await expect(page).toHaveURL(/\/signin/);
  await signIn(page);
  await expect(page).toHaveURL(`${appURL}/devices`);
  await expect(page.getByRole("heading", { name: /devices/i }).first()).toBeVisible();
  await page.reload();
  await expect(page).toHaveURL(`${appURL}/devices`);
  await expect(page.getByRole("heading", { name: /devices/i }).first()).toBeVisible();
});

test.describe("session renewal", () => {
  async function expireAccessToken(page: Page, keepRefresh = true) {
    return page.evaluate((keepRefresh) => {
      const key = Object.keys(localStorage).find((key) => key.startsWith("oidc.token:"));
      if (!key) throw new Error("Sign-in did not store a session");
      const token = JSON.parse(localStorage.getItem(key) ?? "null");
      if (!token?.refresh_token) throw new Error("Sign-in did not issue a refresh token");
      token.access_token_expires_at = 1;
      if (!keepRefresh) token.refresh_token = undefined;
      localStorage.setItem(key, JSON.stringify(token));
      return keepRefresh ? String(token.refresh_token) : undefined;
    }, keepRefresh);
  }

  test("renews once before concurrent Dashboard requests", async ({ page }) => {
    await page.goto(`${appURL}/users`);
    await signIn(page);
    await expect(page.getByRole("heading", { name: "Users", exact: true })).toBeVisible();
    await expect(
      page.getByRole("cell", { name: "admin@local.test", exact: true }).first()
    ).toBeVisible();
    await expireAccessToken(page);

    const grants: string[] = [];
    page.on("request", (request) => {
      if (request.url() !== `${backendURL}/idp/oauth2/token` || request.method() !== "POST") return;
      const grant = new URLSearchParams(request.postData() ?? "").get("grant_type");
      if (grant) grants.push(grant);
    });
    await page.getByRole("link", { name: "Dashboard", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Dashboard" })).toBeVisible();
    await expect(page.getByText("Loading dashboard...", { exact: true })).toHaveCount(0);
    await expect(page.locator("p").and(page.getByText("Users", { exact: true }))).toBeVisible();
    await expect(page).toHaveURL(`${appURL}/`);
    expect(grants[0]).toBe("refresh_token");
    expect(grants.filter((grant) => grant === "refresh_token")).toHaveLength(1);
    expect(grants.filter((grant) => grant.endsWith(":token-exchange"))).toHaveLength(2);
  });

  for (const reload of [false, true]) {
    test(`clears a nonrenewable session and returns to the protected route (${reload ? "reload" : "navigation"})`, async ({
      page,
      request,
    }) => {
      const users = "/users?from=renewal#records";
      await page.goto(`${appURL}${users}`);
      await signIn(page);
      await expect(
        page.getByRole("cell", { name: "admin@local.test", exact: true }).first()
      ).toBeVisible();
      const refresh = await expireAccessToken(page, reload);
      if (refresh) {
        const revoked = await request.post(`${backendURL}/idp/oauth2/revoke`, {
          form: { token: refresh, token_type_hint: "refresh_token" },
        });
        expect(revoked.ok()).toBe(true);
      }

      const exchanges: string[] = [];
      page.on("request", (request) => {
        if (request.url() === `${backendURL}/idp/oauth2/token` && request.method() === "POST")
          exchanges.push(new URLSearchParams(request.postData() ?? "").get("grant_type") ?? "");
      });
      const path = reload ? users : "/";
      if (reload) await page.reload();
      else await page.getByRole("link", { name: "Dashboard", exact: true }).click();
      await expect(page).toHaveURL(`${appURL}/signin`);
      await expect(page.getByRole("button", { name: "Sign out" })).toHaveCount(0);
      expect(exchanges).toEqual(reload ? ["refresh_token"] : []);
      expect(
        await page.evaluate(() =>
          Object.keys(localStorage).some((key) => key.startsWith("oidc.token:"))
        )
      ).toBe(false);

      await signIn(page);
      await expect(page).toHaveURL(`${appURL}${path}`);
      await expect(
        page.getByRole("heading", { name: reload ? "Users" : "Dashboard", exact: true })
      ).toBeVisible();
    });
  }

  test("shows one validation message for each empty sign-in field", async ({ page }) => {
    await page.goto(`${appURL}/signin`);
    await page.getByRole("button", { name: "Sign in", exact: true }).click();
    await expect(page.getByText("Enter your username.", { exact: true })).toHaveCount(1);
    await expect(page.getByText("Enter your password.", { exact: true })).toHaveCount(1);
    await expect(page.getByText(/password must have/)).toHaveCount(0);
  });
});

test("authorization errors give safe, actionable guidance without client registration", async ({
  page,
}) => {
  await page.goto(`${appURL}/signin`);
  await signIn(page);
  await expect(page).not.toHaveURL(/\/signin/);
  const url = new URL(`${appURL}/authorize`);
  url.search = new URLSearchParams({
    response_type: "code",
    response_mode: "query",
    redirect_uri: `${appURL}/callback`,
    scope: "openid",
  }).toString();
  await page.goto(url.href);
  await expect(page.getByRole("heading", { name: "Invalid Request", exact: true })).toBeVisible();
  await expect(page.getByRole("alert")).toContainText(
    "New clients require application_id and registration metadata"
  );

  url.searchParams.set("application_id", "https://other.test");
  url.searchParams.set(
    "registration",
    JSON.stringify({
      application: { uri: "https://app.test" },
      clientName: "Untrusted client",
      profile: "native_application",
      tokenEndpointAuthMethod: "none",
    })
  );
  await page.goto(url.href);
  await expect(page.getByRole("alert")).toContainText(
    "application_id must match registration.application.uri"
  );

  url.searchParams.delete("application_id");
  url.searchParams.delete("registration");
  url.searchParams.set("client_id", "unknown");
  await page.goto(url.href);
  await expect(
    page.getByText("The client ID you provided does not exist.", {
      exact: false,
    })
  ).toBeVisible();
  await expect(
    page.getByText("These details were supplied by the application", {
      exact: false,
    })
  ).toHaveCount(0);
  await expect(page.getByRole("button", { name: /accept|register/i })).toHaveCount(0);
});

test("app signs in and uses its session for Device and Storage access", async ({
  page,
  request,
}) => {
  await page.goto(`${appURL}/signin`);
  const [response] = await Promise.all([
    page.waitForResponse(
      (response) =>
        response.url() === `${backendURL}/idp/oauth2/token` &&
        response.request().postData()?.includes("grant_type=password") === true
    ),
    signIn(page),
  ]);
  expect(response.ok()).toBeTruthy();
  const token = await response.json();
  await expect(page).not.toHaveURL(/\/signin/);
  await page.goto(`${appURL}/devices`);
  await expect(page.getByRole("heading", { name: /devices/i }).first()).toBeVisible();
  const resourceToken = await request.post(`${backendURL}/idp/oauth2/token`, {
    form: {
      grant_type: "urn:ietf:params:oauth:grant-type:token-exchange",
      subject_token: token.access_token,
      subject_token_type: "urn:ietf:params:oauth:token-type:access_token",
      resource: `${backendURL}/device`,
      client_id: "local-login",
    },
  });
  expect(resourceToken.ok()).toBeTruthy();
  const devices = await request.get(`${backendURL}/device/devices`, {
    headers: {
      Authorization: `Bearer ${(await resourceToken.json()).access_token}`,
    },
  });
  expect(await json(devices)).toEqual(expect.any(Array));
  const storage = storageClient("local-login", token.access_token);
  expect(await storage.listDatabases()).toEqual(expect.any(Array));
  await page.reload();
  await expect(page.getByRole("heading", { name: /devices/i }).first()).toBeVisible();
});

test("Devices keeps administration without using legacy sync preferences", async ({ page }) => {
  const legacyEntries = {
    "sync-target-url": JSON.stringify("https://old-storage.example/storage"),
    "sync-preferences:legacy": JSON.stringify({ direction: "Pull", deviceIds: [], results: {} }),
  };
  await page.addInitScript(
    ({ origin, entries }) => {
      if (location.origin !== origin) return;
      for (const [key, value] of Object.entries(entries)) {
        if (localStorage.getItem(key) === null) localStorage.setItem(key, value);
      }
      for (const method of ["getItem", "setItem", "removeItem"] as const) {
        const original = Storage.prototype[method];
        Storage.prototype[method] = function (this: Storage, key: string, ...args: string[]) {
          if (key === "sync-target-url" || key.startsWith("sync-preferences:")) {
            performance.mark(`legacy-sync-storage:${method}:${key}`);
          }
          return Reflect.apply(original, this, [key, ...args]);
        };
      }
    },
    { origin: new URL(appURL).origin, entries: legacyEntries }
  );

  await page.goto(`${appURL}/devices`);
  await expect(page).toHaveURL(/\/signin/);
  await signIn(page);
  await expect(page).toHaveURL(`${appURL}/devices`);

  await expect(page.getByRole("heading", { name: "Sync", exact: true })).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: /^(Sync|Configure sync|Check selected targets)$/ })
  ).toHaveCount(0);
  await expect(
    page.getByText(/Sign in before checking sync|Blocked — no data transferred/)
  ).toHaveCount(0);
  await expect(page.getByRole("combobox", { name: "Direction", exact: true })).toHaveCount(0);

  await expect(page.getByRole("button", { name: "Refresh", exact: true })).toBeEnabled();
  const records = page.waitForResponse(`${backendURL}/device/devices`);
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  const response = await records;
  expect(response.ok()).toBeTruthy();
  expect(await response.json()).toEqual(expect.any(Array));
  await page.getByRole("button", { name: "Add device", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "Enroll New Device" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("button", { name: "Read current restriction" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Select authorized resource" })).toBeVisible();
  await page.getByRole("combobox", { name: "Resource kind" }).selectOption({ label: "Filesystem" });
  await expect(page.getByRole("group", { name: "Local whole-filesystem Residency" })).toBeVisible();

  expect(
    await page.evaluate(() =>
      performance
        .getEntriesByType("mark")
        .map(({ name }) => name)
        .filter((name) => name.startsWith("legacy-sync-storage:"))
    )
  ).toEqual([]);
  expect(
    await page.evaluate(
      (keys) => Object.fromEntries(keys.map((key) => [key, localStorage.getItem(key)])),
      Object.keys(legacyEntries)
    )
  ).toEqual(legacyEntries);
});
