import { expect, type Page, test } from "@playwright/test";
import { appURL, backendURL, passwordManagerURL } from "./backend.mjs";
import { signIn } from "./session";

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
  await expect(page.getByRole("button", { name: "Show synced vaults", exact: true })).toBeVisible();
});

test("synced vault network failure gives service recovery without local-storage advice", async ({
  page,
}) => {
  await signInToSync(page);
  const allow = page.getByRole("button", { name: "Allow", exact: true });
  const signedIn = page.getByRole("button", { name: "Create synced vault", exact: true });
  await expect(allow.or(signedIn)).toBeVisible();
  if (await allow.isVisible()) await allow.click();
  await expect(page).toHaveURL(`${passwordManagerURL}/`);
  await page.route(`${backendURL}/idp/oauth2/token`, async (route) => {
    const grant = new URLSearchParams(route.request().postData() ?? "").get("grant_type");
    if (grant === "urn:ietf:params:oauth:grant-type:token-exchange") await route.abort("failed");
    else await route.continue();
  });

  await page.getByRole("button", { name: "Create synced vault", exact: true }).click();
  await page.getByLabel("Vault name", { exact: true }).fill("Interrupted sync vault");
  await page.getByLabel("Master password", { exact: true }).fill("synthetic-sync-password");
  await page.getByLabel("Confirm password", { exact: true }).fill("synthetic-sync-password");
  await page.getByRole("button", { name: "Create vault", exact: true }).click();

  const alert = page.getByRole("alert");
  await expect(alert).toContainText(
    "Storage service is unavailable. Check the Node status and connection, then retry."
  );
  await expect(alert).toContainText("Return to the selector");
  await expect(alert).not.toContainText("interceptors");
  await expect(alert).not.toContainText("Check browser storage");
});
