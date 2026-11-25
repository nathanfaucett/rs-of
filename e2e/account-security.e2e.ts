import { expect, test } from "@playwright/test";
import { appURL, backendURL } from "./backend.mjs";
import { json, signIn } from "./session";

const username = "account-security-fixture";

test.beforeEach(async ({ page, request }) => {
  const login = await json<{ access_token: string }>(
    await request.post(`${backendURL}/idp/oauth2/token`, {
      form: {
        grant_type: "password",
        client_id: "local-login",
        username,
        password: "account-security-password",
        scope: "openid profile email",
      },
    })
  );
  const user = await json<{ sub: string }>(
    await request.get(`${backendURL}/idp/userinfo`, {
      headers: { Authorization: `Bearer ${login.access_token}` },
    })
  );
  const url = `${appURL}/users/${user.sub}`;
  await page.goto(url);
  await expect(page).toHaveURL(/\/signin/);
  await signIn(page);
  await expect(page).toHaveURL(url);
  await expect(page.getByText("Active", { exact: true })).toBeVisible();
  await expect(page.getByText("Disabled", { exact: true })).toBeVisible();
});

test("account security: cancel sends no Block request and confirmed denial keeps the User active", async ({
  page,
}) => {
  const block = page.getByRole("button", { name: "Block", exact: true });
  let requests = 0;
  page.on("request", (request) => {
    if (request.method() === "POST" && request.url().endsWith("/block")) requests++;
  });
  let confirmation = "";
  page.once("dialog", async (dialog) => {
    confirmation = dialog.message();
    await dialog.dismiss();
  });
  await block.click();
  expect(confirmation).toContain(username);
  expect(requests).toBe(0);
  await expect(page.getByText("Active", { exact: true })).toBeVisible();

  page.once("dialog", (dialog) => dialog.accept());
  const denied = page.waitForResponse((response) => response.url().endsWith("/block"));
  await block.click();
  expect((await denied).status()).toBe(403);
  await expect(page.getByRole("alert").filter({ hasText: "idp.users.block" })).toBeVisible();
  await expect(page.getByText("Active", { exact: true })).toBeVisible();
  await expect(block).toBeEnabled();
  expect(requests).toBe(1);
});

test("account security: MFA denial and connection failure keep TOTP disabled", async ({ page }) => {
  const enable = page.getByRole("button", { name: "Enable", exact: true });
  const denied = page.waitForResponse((response) => response.url().endsWith("/mfa"));
  await enable.click();
  expect((await denied).status()).toBe(403);
  await expect(page.getByRole("alert").filter({ hasText: "idp.users.mfa" })).toBeVisible();
  await expect(page.getByText("Disabled", { exact: true })).toBeVisible();

  // Interrupt only the mutation; sign-in, reads, and authorization above use the real IdP.
  await page.route(`${backendURL}/idp/users/*/mfa`, (route) => route.abort("failed"));
  await enable.click();
  await expect(page.getByRole("alert").filter({ hasText: "Check the connection" })).toBeVisible();
  await expect(page.getByText("Disabled", { exact: true })).toBeVisible();
  await expect(enable).toBeEnabled();
});
