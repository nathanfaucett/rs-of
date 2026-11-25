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
    await page.getByRole("link", { name: `${appURL}/local-login`, exact: true }).click();
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
