import { type ConsoleMessage, expect, type Page, test } from "@playwright/test";
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

test("management routes identify their page and recover from a missing route", async ({ page }) => {
  const errors: string[] = [];
  const fetchWarnings: string[] = [];
  const recordConsoleErrors = (message: ConsoleMessage) => {
    if (message.type() === "error") errors.push(message.text());
  };
  page.on("console", recordConsoleErrors);
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "warning" && message.text().includes("`window.fetch`"))
      fetchWarnings.push(message.text());
  });

  await page.goto(`${appURL}/signin`);
  await expect(page).toHaveTitle("Sign in — of");
  await signIn(page);
  await expect(page).toHaveTitle("Dashboard — of");

  // Other tickets own the known backend errors on list and detail routes.
  page.off("console", recordConsoleErrors);
  await page.getByRole("link", { name: "Applications", exact: true }).click();
  await expect(page).toHaveTitle("Applications — of");
  await expect(page.getByRole("main")).toBeFocused();
  await page
    .getByRole("row", { name: /local-login/ })
    .getByRole("link", { name: /[0-9a-f-]{36}/ })
    .click();
  await expect(page).toHaveTitle("local-login — Application — of");
  await page.goBack();
  await expect(page).toHaveTitle("Applications — of");

  await page.goto(`${appURL}/clients`);
  await expect(page).toHaveTitle("Clients — of");
  await page.goto(`${appURL}/clients/local-login`);
  await expect(page).toHaveTitle("local-login — Client — of");

  await page.getByRole("link", { name: "Users", exact: true }).click();
  await expect(page).toHaveTitle("Users — of");
  await page
    .getByRole("row", { name: /admin@local[.]test/ })
    .getByRole("link", { name: /[0-9a-f-]{36}/ })
    .click();
  await expect(page).toHaveTitle("admin@local.test — User — of");

  await page.getByRole("link", { name: "Devices", exact: true }).click();
  await expect(page).toHaveTitle("Devices — of");
  await page.goto(`${appURL}/roles`);
  await expect(page).toHaveTitle("Roles — of");

  await page.goto(`${appURL}/missing-management-page`);
  await expect(page).toHaveTitle(/^404 —/);
  page.on("console", recordConsoleErrors);
  await page.getByRole("link", { name: "Dashboard", exact: true }).click();
  await expect(page).toHaveURL(`${appURL}/`);
  await expect(page).toHaveTitle("Dashboard — of");
  expect(fetchWarnings).toEqual([]);
  expect(errors).toEqual([]);
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

test("OAuth Client lookup explains a cross-Application denial", async ({ page }) => {
  await page.goto(`${appURL}/clients`);
  await signIn(page);
  await page.getByRole("textbox", { name: "Client ID", exact: true }).fill("password-manager");
  const lookup = page.waitForResponse(`${backendURL}/idp/oauth2/register/password-manager`);
  await page.getByRole("button", { name: "Open client", exact: true }).click();
  expect((await lookup).status()).toBe(403);
  await expect(page.getByRole("alert")).toContainText(
    "You do not have permission to view this client."
  );
  await page.getByRole("link", { name: "Back to client lookup" }).click();
  await expect(page).toHaveURL(`${appURL}/clients`);
});

test("OAuth Client lookup distinguishes an unknown ID from a service failure", async ({ page }) => {
  await page.goto(`${appURL}/clients/unknown-client`);
  const missing = page.waitForResponse(`${backendURL}/idp/oauth2/register/unknown-client`);
  await signIn(page);
  expect((await missing).status()).toBe(400);
  await expect(page.getByRole("alert")).toContainText("Client not found.");

  // Fail only the transport. The retry loads a real registration from the IdP.
  await page.route(`${backendURL}/idp/oauth2/register/local-management`, (route) => route.abort(), {
    times: 1,
  });
  await page.goto(`${appURL}/clients/local-management`);
  await expect(page.getByRole("alert")).toContainText("Could not load the client.");
  await page.getByRole("button", { name: "Retry lookup" }).click();
  await expect(page.getByRole("textbox", { name: "Name", exact: true })).toHaveValue(
    "local-management"
  );
});

test("OAuth Client creation keeps only session metadata and reopens the owner registration", async ({
  page,
}) => {
  await page.goto(`${appURL}/applications`);
  await signIn(page);
  const openApplication = async () => {
    await page.getByRole("link", { name: "local-login", exact: true }).click();
    await expect(page).toHaveURL(/\/applications\/[^/]+$/);
    await expect(page.getByText(/Reloading clears this list/)).toBeVisible();
  };
  await openApplication();
  const applicationURL = page.url();

  // The isolated backend removes the synthetic registrations at teardown.
  const createClient = async (name: string) => {
    await page.getByRole("textbox", { name: "Client name", exact: true }).fill(name);
    await page
      .getByRole("textbox", { name: "Redirect URIs", exact: true })
      .fill(`${appURL}/callback`);
    const registered = page.waitForResponse(
      (response) =>
        response.url() === `${backendURL}/idp/oauth2/register` &&
        response.request().method() === "POST"
    );
    await page.getByRole("button", { name: "Create client", exact: true }).click();
    const response = await registered;
    expect(response.status()).toBe(200);
    const {
      client_id,
      client_type,
      client_secret,
    }: { client_id: string; client_type: string; client_secret?: string } = await response.json();
    expect(client_type).toBe("public");
    expect(client_secret).toBeUndefined();
    const link = page
      .getByRole("row")
      .filter({ hasText: name })
      .getByRole("link", { name: client_id, exact: true });
    await expect(link).toBeVisible();
    return { id: client_id, link };
  };

  const name = "E2E OAuth Client";
  const { id, link } = await createClient(name);
  expect(
    await page.evaluate(
      (id) =>
        [localStorage, sessionStorage].some((storage) =>
          Object.values(storage).some((value) => String(value).includes(id))
        ),
      id
    )
  ).toBe(false);
  const lookup = page.waitForResponse(`${backendURL}/idp/oauth2/register/${id}`);
  await link.click();
  expect((await lookup).status()).toBe(200);
  const clientName = page.getByRole("textbox", { name: "Name", exact: true });
  await expect(clientName).toHaveValue(name);
  await page.goBack();
  await expect(link).toBeVisible();
  await link.click();
  await page.getByRole("link", { name: "Back to application" }).click();
  await expect(link).toBeVisible();

  const freshPage = await page.context().newPage();
  await freshPage.goto(applicationURL);
  await expect(freshPage.getByText(/Reloading clears this list/)).toBeVisible();
  await expect(freshPage.getByRole("link", { name: id, exact: true })).toHaveCount(0);
  await freshPage.close();

  await page.getByRole("button", { name: "Sign out", exact: true }).click();
  await signIn(page);
  await page.getByRole("link", { name: "Applications", exact: true }).click();
  await openApplication();
  await expect(link).toHaveCount(0);

  const replacement = await createClient("E2E Client before session replacement");
  const otherTab = await page.context().newPage();
  await otherTab.goto(`${appURL}/signin`);
  await signIn(otherTab);
  await expect(otherTab).not.toHaveURL(/\/signin/);
  await expect(replacement.link).toHaveCount(0);
  await otherTab.close();

  await page.getByRole("link", { name: /look up a client by ID/i }).click();
  await page.getByRole("textbox", { name: "Client ID", exact: true }).fill(id);
  await page.getByRole("button", { name: "Open client", exact: true }).click();
  await expect(clientName).toHaveValue(name);
  await page.reload();
  await expect(clientName).toHaveValue(name);
});

test("management navigation works by keyboard and remembers both layouts", async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 780 });
  await page.goto(`${appURL}/`);
  await signIn(page);

  const menu = page.getByRole("button", { name: "Navigation", exact: true });
  const navigation = page.getByRole("navigation", { name: "Management" });
  await expect(menu).toHaveAttribute("aria-expanded", "false");
  await expect(navigation).toBeHidden();
  await menu.focus();
  await menu.press("Enter");
  await expect(menu).toHaveAttribute("aria-expanded", "true");
  await page.keyboard.press("Tab");
  const dashboard = navigation.getByRole("link", { name: "Dashboard" });
  await expect(dashboard).toBeFocused();
  await expect(dashboard).toHaveCSS("outline-style", "solid");
  await page.keyboard.press("Escape");
  await expect(menu).toBeFocused();
  await expect(menu).toHaveAttribute("aria-expanded", "false");

  await menu.press("Enter");
  const applications = navigation.getByRole("link", { name: "Applications" });
  await applications.press("Enter");
  await expect(page).toHaveURL(`${appURL}/applications`);
  await expect(page.getByRole("main")).toBeFocused();
  await expect(applications).toHaveAttribute("aria-current", "page");
  await page.reload();
  await expect(menu).toHaveAttribute("aria-expanded", "true");

  await page.setViewportSize({ width: 1440, height: 780 });
  await page.getByRole("button", { name: "Collapse sidebar" }).press("Enter");
  await navigation.getByRole("link", { name: "Users" }).click();
  await expect(page).toHaveURL(`${appURL}/users`);
  await page.reload();
  await expect(page.getByRole("button", { name: "Expand sidebar" })).toBeVisible();
  await expect(navigation.getByRole("link", { name: "Users" })).toHaveAttribute(
    "aria-current",
    "page"
  );
});

for (const width of [320, 390, 768, 1440]) {
  test(`management forms and records fit at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 780 });
    await page.goto(`${appURL}/applications`);
    await signIn(page);
    await expect(page).toHaveURL(`${appURL}/applications`);

    const main = page.getByRole("main");
    const configuration = page.getByRole("button", { name: "Configuration", exact: true });

    async function expectRecordToFit() {
      const record = main
        .getByRole("row")
        .filter({ has: page.getByRole("cell") })
        .first();
      await expect(record).toBeVisible();
      const cells = await record.getByRole("cell").evaluateAll((elements) =>
        elements.map((cell) => {
          const { left, right } = cell.getBoundingClientRect();
          return { left, right, scroll: cell.scrollWidth, width: cell.clientWidth };
        })
      );
      expect(cells.length).toBeGreaterThan(0);
      for (const cell of cells) {
        expect(cell.left).toBeGreaterThanOrEqual(0);
        expect(cell.right).toBeLessThanOrEqual(width);
        expect(cell.scroll).toBeLessThanOrEqual(cell.width);
      }
    }

    async function expectDiscoveryToFit(scope: string) {
      const search = main.getByRole("searchbox", { name: `Search ${scope}` });
      const sort = main.getByRole("combobox", { name: `Sort ${scope}` });
      await search.press("Tab");
      await expect(sort).toBeFocused();
      for (const control of [search, sort]) {
        const box = await control.boundingBox();
        if (!box) throw new Error("List discovery control is not visible");
        expect(box.x).toBeGreaterThanOrEqual(0);
        expect(box.x + box.width).toBeLessThanOrEqual(width);
      }
    }

    await expectDiscoveryToFit("current page");
    await main.getByRole("textbox", { name: "Name" }).fill("Responsive draft");
    await main.getByRole("textbox", { name: "URI" }).fill("https://responsive.example.test");
    await main.getByRole("button", { name: "Create application" }).click({ trial: true });
    await expectRecordToFit();

    await page.goto(`${appURL}/users`);
    await expectDiscoveryToFit("current page");
    await expectRecordToFit();
    await main.getByRole("button", { name: "Add user" }).click();
    const createUser = page.getByRole("dialog");
    await createUser.getByRole("textbox", { name: "Name" }).fill("Responsive draft");
    await createUser.getByRole("button", { name: "Cancel" }).click();
    await expect(createUser).toBeHidden();

    await page.goto(`${appURL}/devices`);
    await expectDiscoveryToFit("loaded devices");
    const refreshed = page.waitForResponse(`${backendURL}/device/devices`);
    await main.getByRole("button", { name: "Refresh", exact: true }).click();
    expect((await refreshed).ok()).toBeTruthy();
    await main.getByRole("button", { name: "Add device" }).click();
    const enrollDevice = page.getByRole("dialog");
    await enrollDevice.getByRole("textbox", { name: "Device name" }).fill("Responsive draft");
    await enrollDevice.getByRole("button", { name: "Enroll device" }).click({ trial: true });
    await enrollDevice.getByRole("button", { name: "Cancel" }).click();
    await expect(enrollDevice).toBeHidden();

    const mainBox = await main.boundingBox();
    const configurationBox = await configuration.boundingBox();
    if (!mainBox || !configurationBox) throw new Error("Management controls are not visible");
    expect(
      configurationBox.y >= mainBox.y + mainBox.height ||
        configurationBox.y + configurationBox.height <= mainBox.y ||
        configurationBox.x >= mainBox.x + mainBox.width ||
        configurationBox.x + configurationBox.width <= mainBox.x
    ).toBe(true);
    expect(await main.evaluate((element) => element.scrollWidth)).toBeLessThanOrEqual(
      await main.evaluate((element) => element.clientWidth)
    );
    await configuration.click();
    await expect(page.getByRole("dialog")).toBeVisible();
  });
}
