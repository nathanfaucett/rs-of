import { expect, test } from "@playwright/test";
import { appURL, backendURL, passwordManagerURL } from "./backend.mjs";
import { signIn } from "./session";

test("password-manager redirects to IdP, requests consent, and establishes a session", async ({
  page,
}) => {
  await page.goto(`${passwordManagerURL}/login`);
  await page.getByRole("button", { name: "Sign in with IdP", exact: true }).click();
  await expect(page).toHaveURL(new RegExp(`^${appURL}/signin`));
  await signIn(page);
  await expect(page).toHaveURL(new RegExp(`^${appURL}/authorize`));
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
});
