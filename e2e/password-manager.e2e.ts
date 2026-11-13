import { expect, test, type Page } from "@playwright/test";
import { appURL, backendURL, passwordManagerURL } from "./backend.mjs";
import { signIn } from "./session";

async function requestConsent(page: Page) {
  await page.goto(`${passwordManagerURL}/login`);
  await page
    .getByRole("button", { name: "Sign in with IdP", exact: true })
    .click();
  await expect(page).toHaveURL(new RegExp(`^${appURL}/signin`));
  await signIn(page);
  await expect(page).toHaveURL(new RegExp(`^${appURL}/authorize`));
  await expect(
    page.getByRole("heading", { name: "password-manager", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("listitem").filter({ hasText: /^storage$/ }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Allow", exact: true }),
  ).toBeEnabled();
  await expect(
    page.getByRole("button", { name: "Deny", exact: true }),
  ).toBeEnabled();
}

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
  expect(
    exchanges.filter((body) => body.includes("grant_type=authorization_code")),
  ).toEqual([]);
});

test("password-manager redirects to IdP, requests consent, and establishes a session", async ({
  page,
}) => {
  await requestConsent(page);
  const consent = new URL(page.url());
  consent.searchParams.set("registration", "malformed-untrusted-registration");
  await page.goto(consent.href);
  await expect(
    page.getByRole("heading", { name: "password-manager", exact: true }),
  ).toBeVisible();
  await expect(page.getByText("malformed-untrusted-registration")).toHaveCount(
    0,
  );
  await expect(
    page.getByText("Invalid registration metadata", { exact: false }),
  ).toHaveCount(0);
  await expect(
    page.getByText("These details were supplied by the application", {
      exact: false,
    }),
  ).toHaveCount(0);
  const [approval, response, user] = await Promise.all([
    page.waitForResponse(
      (response) =>
        response.url() === `${backendURL}/idp/oauth2/auth/approve-for-user` &&
        response.request().method() === "POST",
    ),
    page.waitForResponse(
      (response) =>
        response.url() === `${backendURL}/idp/oauth2/token` &&
        response
          .request()
          .postData()
          ?.includes("grant_type=authorization_code") === true,
    ),
    page.waitForResponse(
      (response) => response.url() === `${backendURL}/idp/userinfo`,
    ),
    page.getByRole("button", { name: "Allow", exact: true }).click(),
  ]);
  expect(approval.ok()).toBeTruthy();
  expect(response.ok()).toBeTruthy();
  expect(await response.json()).toEqual(
    expect.objectContaining({
      access_token: expect.any(String),
      id_token: expect.any(String),
    }),
  );
  await expect(page).toHaveURL(`${passwordManagerURL}/`);
  await expect(
    page.getByRole("button", { name: "Create new vault", exact: true }),
  ).toBeVisible();

  expect(user.ok()).toBeTruthy();
  expect((await user.json()).email).toBe("admin@local.test");
  await page.reload();
  await expect(page).toHaveURL(`${passwordManagerURL}/`);
  await expect(
    page.getByRole("button", { name: "Create new vault", exact: true }),
  ).toBeVisible();
});
