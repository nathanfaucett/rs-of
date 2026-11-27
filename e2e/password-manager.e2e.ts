import { expect, type Page, test } from "@playwright/test";
import { frame } from "../packages/storage-sync-client/src/protocol";
import { appURL, backendURL, passwordManagerURL } from "./backend.mjs";
import { frontendSession, signIn, storageClient } from "./session";

async function signInToSync(page: Page) {
  await page.goto(passwordManagerURL);
  await page.getByRole("link", { name: "Sign in to sync", exact: true }).click();
  await page.getByRole("button", { name: "Sign in with IdP", exact: true }).click();
  await expect(page).toHaveURL(new RegExp(`^${appURL}/signin`));
  await signIn(page);
}

async function requestConsent(page: Page) {
  await signInToSync(page);
  await expect(page).toHaveURL(new RegExp(`^${appURL}/authorize`));
  await expect(page.getByRole("heading", { name: "password-manager", exact: true })).toBeVisible();
  await expect(page.getByRole("listitem").filter({ hasText: /^storage$/ })).toBeVisible();
  await expect(page.getByRole("button", { name: "Allow", exact: true })).toBeEnabled();
  await expect(page.getByRole("button", { name: "Deny", exact: true })).toBeEnabled();
}

test("sign in to sync is in the navigation sidebar for an unlocked local vault", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto(passwordManagerURL);
  await page.getByRole("button", { name: "Create new vault", exact: true }).click();
  await page.getByLabel(/^Vault name/).fill("Sidebar test vault");
  await page.getByLabel(/^Master password/).fill("synthetic-sidebar-password");
  await page.getByLabel(/^Confirm password/).fill("synthetic-sidebar-password");
  await page.getByRole("button", { name: "Create vault", exact: true }).click();
  await expect(page).toHaveURL(`${passwordManagerURL}/vault`);

  const signInLink = { name: "Sign in to sync", exact: true };
  const navigation = page.getByRole("navigation", { name: "Main navigation" });
  await expect(navigation.getByRole("link", signInLink)).toBeVisible();
  await expect(page.getByRole("link", { ...signInLink, includeHidden: true })).toHaveCount(1);

  await page.getByRole("button", { name: "Collapse navigation" }).click();
  await page.setViewportSize({ width: 390, height: 800 });
  await page.getByRole("button", { name: "Open navigation" }).click();
  await navigation.getByRole("link", signInLink).click();
  await expect(page).toHaveURL(`${passwordManagerURL}/login`);
  await expect(page.getByRole("button", { name: "Sign in with IdP" })).toBeVisible();

  await page.goBack();
  await page.getByRole("button", { name: "Open navigation" }).click();
  const lock = navigation.getByRole("button", { name: "Lock vault" });
  await lock.press("Enter");
  await expect(page).toHaveURL(`${passwordManagerURL}/`);
  await expect(lock).toHaveCount(0);
});

test("denying real consent returns access_denied with the original state and no token exchange", async ({
  page,
}) => {
  await requestConsent(page);
  const state = new URL(page.url()).searchParams.get("state");
  expect(state).toBeTruthy();
  const exchanges: string[] = [];
  page.on("request", (request) => {
    if (request.url() === `${backendURL}/idp/oauth2/token`)
      exchanges.push(request.postData() ?? "");
  });
  await page.getByRole("button", { name: "Deny", exact: true }).click();
  await expect(page).toHaveURL(new RegExp(`^${passwordManagerURL}/callback`));
  const callback = new URL(page.url());
  expect(callback.searchParams.get("error")).toBe("access_denied");
  expect(callback.searchParams.get("state")).toBe(state);
  await expect(page.getByText("access_denied", { exact: true })).toBeVisible();
  expect(exchanges.filter((body) => body.includes("grant_type=authorization_code"))).toEqual([]);
});

test("password-manager redirects to IdP, requests consent, and establishes a session", async ({
  page,
}) => {
  await requestConsent(page);
  const consent = new URL(page.url());
  consent.searchParams.set("registration", "malformed-untrusted-registration");
  await page.goto(consent.href);
  await expect(page.getByRole("heading", { name: "password-manager", exact: true })).toBeVisible();
  await expect(page.getByText("malformed-untrusted-registration")).toHaveCount(0);
  await expect(page.getByText("Invalid registration metadata", { exact: false })).toHaveCount(0);
  await expect(
    page.getByText("These details were supplied by the application", {
      exact: false,
    })
  ).toHaveCount(0);
  const [approval, response, user] = await Promise.all([
    page.waitForResponse(
      (response) =>
        response.url() === `${backendURL}/idp/oauth2/auth/approve-for-user` &&
        response.request().method() === "POST"
    ),
    page.waitForResponse(
      (response) =>
        response.url() === `${backendURL}/idp/oauth2/token` &&
        response.request().postData()?.includes("grant_type=authorization_code") === true
    ),
    page.waitForResponse((response) => response.url() === `${backendURL}/idp/userinfo`),
    page.getByRole("button", { name: "Allow", exact: true }).click(),
  ]);
  expect(approval.ok()).toBeTruthy();
  expect(response.ok()).toBeTruthy();
  expect(await response.json()).toEqual(
    expect.objectContaining({
      access_token: expect.any(String),
      id_token: expect.any(String),
    })
  );
  await expect(page).toHaveURL(`${passwordManagerURL}/`);
  await expect(page.getByRole("button", { name: "Create new vault", exact: true })).toBeVisible();

  expect(user.ok()).toBeTruthy();
  expect((await user.json()).email).toBe("admin@local.test");
  await page.reload();
  await expect(page).toHaveURL(`${passwordManagerURL}/`);
  await expect(page.getByRole("button", { name: "Find remote database", exact: true })).toBeVisible();
});

test("first upload stays guarded after a lost publish receipt", async ({ page, request }) => {
  await signInToSync(page);
  const allow = page.getByRole("button", { name: "Allow", exact: true });
  const create = page.getByRole("button", { name: "Create new vault", exact: true });
  await expect(allow.or(create)).toBeVisible();
  if (await allow.isVisible()) await allow.click();
  await expect(page).toHaveURL(`${passwordManagerURL}/`);
  await create.click();
  await page.getByLabel("Vault name", { exact: true }).fill("Local vault");
  const masterPassword = "synthetic-sync-password";
  await page.getByLabel("Master password", { exact: true }).fill(masterPassword);
  await page.getByLabel("Confirm password", { exact: true }).fill(masterPassword);
  await page.getByRole("button", { name: "Create vault", exact: true }).click();
  const plaintext = "A local note for guarded first upload.";
  await page.getByRole("button", { name: "New item", exact: true }).click();
  await page.getByRole("menuitem", { name: "New secure note" }).click();
  await page.getByRole("textbox", { name: "Name", exact: true }).fill("Local note");
  await page.getByLabel("Notes", { exact: true }).fill(plaintext);
  await page.getByRole("button", { name: "Create item" }).click();
  await expect(page.getByRole("button", { name: "Local note", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Lock vault" }).click();
  await expect(page.getByRole("button", { name: "Create synced vault" })).toHaveCount(0);
  const tokenEndpoint = `${backendURL}/idp/oauth2/token`;
  await page.route(tokenEndpoint, async (route) => {
    const grant = new URLSearchParams(route.request().postData() ?? "").get("grant_type");
    if (grant === "urn:ietf:params:oauth:grant-type:token-exchange") await route.abort("failed");
    else await route.continue();
  });

  const connect = page.getByRole("button", { name: "Connect local database" });
  const destination = page.getByRole("region", { name: "Confirm sync destination" });
  await connect.click();
  await destination.getByRole("button", { name: "Cancel" }).click();
  await expect(destination).toBeHidden();
  await connect.click();
  await destination.getByRole("button", { name: "Confirm and upload" }).click();
  await expect(destination.getByRole("alert")).toContainText(
    "Storage service is unavailable. Check the Node status and connection, then retry."
  );
  await destination.getByRole("button", { name: "Cancel" }).click();
  await page.getByRole("button", { name: /Local vault/ }).click();
  await page.getByLabel("Master password", { exact: true }).fill(masterPassword);
  await page.getByRole("button", { name: "Unlock", exact: true }).click();
  await expect(page).toHaveURL(`${passwordManagerURL}/vault`);

  const session = await frontendSession(request, "password-manager", passwordManagerURL);
  const storage = storageClient("password-manager", session.access_token);
  const before = await storage.listDatabases();
  await page.unroute(tokenEndpoint);
  // FirstUploadPublished has no body beyond its message tag.
  const publishReceipt = Buffer.from(frame(Uint8Array.of(8)));
  let receiptDropped = false;
  await page.routeWebSocket(`${backendURL.replace(/^http/, "ws")}/storage/sync/ws`, (socket) => {
    const server = socket.connectToServer();
    server.onMessage((message) => {
      if (!receiptDropped && typeof message !== "string" && message.equals(publishReceipt)) {
        receiptDropped = true;
        void socket.close();
        return;
      }
      socket.send(message);
    });
  });
  await page.getByRole("button", { name: "Lock vault" }).click();
  await connect.click();
  await destination.getByRole("button", { name: "Confirm and upload" }).click();
  await expect(destination.getByRole("alert")).toContainText("Storage did not confirm publication");
  expect(receiptDropped).toBe(true);
  await expect(destination.getByRole("status")).toHaveCount(0);
  const after = await storage.listDatabases();
  const created = after.filter(({ id }) => !before.some((resource) => resource.id === id));
  expect(created).toHaveLength(1);
  const resourceId = created[0]?.id;
  if (!resourceId) throw new Error("Missing uploaded Resource");
  await expect(destination).toContainText(resourceId);
  const read = () =>
    storage.executeSql(
      resourceId,
      {
        statements: [{ sql: "SELECT ciphertext, nonce FROM secrets", params: [] }],
      },
      ["read"]
    );
  const encrypted = await read();
  expect(encrypted.results[0]?.rows).toEqual([
    [
      { type: "text", value: expect.stringMatching(/^[A-Za-z0-9+/]+=*$/) },
      { type: "text", value: expect.stringMatching(/^[A-Za-z0-9+/]{16}$/) },
    ],
  ]);
  expect(JSON.stringify(encrypted)).not.toContain(plaintext);
  expect(JSON.stringify(encrypted)).not.toContain(masterPassword);
  await page.reload();
  await expect(page.getByRole("button", { name: "Sync now", exact: true })).toHaveCount(0);
  await connect.click();
  await destination.getByRole("button", { name: "Confirm and upload" }).click();
  await expect(destination.getByRole("alert")).toContainText("already has history");
  const retry = destination.getByRole("button", { name: "Retry first upload" });
  await expect(destination).toContainText(resourceId);
  await expect(retry).toHaveCount(0);
  expect(await storage.listDatabases()).toEqual(after);
  expect(await read()).toEqual(encrypted);
  await destination.getByRole("button", { name: "Cancel" }).click();
  await page.getByRole("button", { name: /Local vault/ }).click();
  await page.getByLabel("Master password", { exact: true }).fill(masterPassword);
  await page.getByRole("button", { name: "Unlock", exact: true }).click();
  await expect(page.getByRole("button", { name: "Local note", exact: true })).toBeVisible();
});
