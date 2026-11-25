import { expect, test } from "@playwright/test";
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

    await main.getByRole("textbox", { name: "Name" }).fill("Responsive draft");
    await main.getByRole("textbox", { name: "URI" }).fill("https://responsive.example.test");
    await main.getByRole("button", { name: "Create application" }).click({ trial: true });
    await expectRecordToFit();

    await page.goto(`${appURL}/users`);
    await expectRecordToFit();
    await main.getByRole("button", { name: "Add user" }).click();
    const createUser = page.getByRole("dialog");
    await createUser.getByRole("textbox", { name: "Name" }).fill("Responsive draft");
    await createUser.getByRole("button", { name: "Cancel" }).click();
    await expect(createUser).toBeHidden();

    await page.goto(`${appURL}/devices`);
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
