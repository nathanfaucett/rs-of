import { randomUUID } from "node:crypto";
import { expect, type Page, test } from "@playwright/test";
import { appURL, backendURL } from "./backend.mjs";
import { json, passwordSession, signIn } from "./session";

async function checkDismissal(
  page: Page,
  triggerName: string,
  title: string,
  dismissal: "Cancel" | "Close" | "Escape"
) {
  const trigger = page.getByRole("button", { name: triggerName, exact: true });
  await trigger.focus();
  await trigger.press("Enter");
  const dialog = page.getByRole("dialog", { name: title, exact: true });
  await expect(dialog).toBeVisible();
  await expect(dialog.locator(":focus")).toBeFocused();
  if (dismissal === "Escape") {
    await page.keyboard.press("Escape");
  } else {
    await dialog.getByRole("button", { name: dismissal, exact: true }).press("Enter");
  }
  await expect(dialog).toBeHidden();
  await expect(trigger).toBeFocused();
}

async function expectDistinctTitleIds(page: Page, minimum: number) {
  const titleIds = await page
    .locator("dialog[aria-labelledby]")
    .evaluateAll((dialogs) => dialogs.map((dialog) => dialog.getAttribute("aria-labelledby")));
  expect(titleIds.length).toBeGreaterThanOrEqual(minimum);
  expect(new Set(titleIds).size).toBe(titleIds.length);
}

test("Configuration and Create User dialogs keep distinct names and return focus", async ({
  page,
}) => {
  await page.goto(`${appURL}/users`);
  await signIn(page);
  await expect(page.getByRole("button", { name: "Add user", exact: true })).toBeVisible();
  await checkDismissal(page, "Configuration", "Configuration", "Escape");
  await checkDismissal(page, "Add user", "Create User", "Cancel");
  await checkDismissal(page, "Add user", "Create User", "Close");
  await expectDistinctTitleIds(page, 2);
});

test("Delete User and Reset Password Cancel dismiss their own dialogs and return focus", async ({
  page,
  request,
}) => {
  const session = await passwordSession(request);
  const user = await json<{ sub: string }>(
    await request.get(`${backendURL}/idp/userinfo`, {
      headers: { Authorization: `Bearer ${session.access_token}` },
    })
  );
  const userUrl = `${backendURL}/idp/users/${user.sub}`;
  const mutations: string[] = [];
  page.on("request", (request) => {
    if (request.url().startsWith(userUrl) && ["DELETE", "PUT"].includes(request.method())) {
      mutations.push(request.method());
    }
  });

  await page.goto(`${appURL}/users/${user.sub}`);
  await signIn(page);
  await checkDismissal(page, "Delete", "Delete User", "Cancel");
  await checkDismissal(page, "Reset password", "Reset Password", "Cancel");
  await expectDistinctTitleIds(page, 3);
  expect(mutations).toEqual([]);
});

test("password reset reports a service failure, then confirms a local reset without email", async ({
  page,
  request,
}) => {
  const session = await passwordSession(request);
  const idp = `${backendURL}/idp`;
  const token = await json<{ access_token: string }>(
    await request.post(`${idp}/oauth2/token`, {
      form: {
        grant_type: "urn:ietf:params:oauth:grant-type:token-exchange",
        subject_token: session.access_token,
        subject_token_type: "urn:ietf:params:oauth:token-type:access_token",
        resource: idp,
      },
    })
  );
  const headers = { Authorization: `Bearer ${token.access_token}` };
  const email = `password-reset-${randomUUID()}@example.test`;
  const original = "original-test-password";
  const replacement = "replacement-test-password";
  const { sub } = await json<{ sub: string }>(
    await request.post(`${idp}/users`, {
      headers,
      data: { email, name: "Reset fixture", password: original },
    })
  );
  const endpoint = `${idp}/users/${sub}/password`;
  const login = (password: string) =>
    request.post(`${idp}/oauth2/token`, {
      form: { grant_type: "password", client_id: "local-login", username: email, password },
    });
  try {
    await page.goto(`${appURL}/users/${sub}`);
    await signIn(page);
    const open = page.getByRole("button", { name: "Reset password", exact: true });
    await open.click();
    const dialog = page.getByRole("dialog", { name: "Reset Password", exact: true });
    const password = dialog.getByLabel(/^New password/);
    const submit = dialog.getByRole("button", { name: "Reset password", exact: true });
    await expect(dialog.getByText(email, { exact: true })).toBeVisible();
    await expect(dialog).toContainText("No email is sent");

    // Inject only the failed mutation; the retry and sign-ins use the real IdP.
    await page.route(
      endpoint,
      (route) => route.fulfill({ status: 500, headers: { "access-control-allow-origin": appURL } }),
      { times: 1 }
    );
    await password.fill(replacement);
    await submit.click();
    await expect(dialog.getByRole("alert")).toContainText("The IdP did not confirm the reset.");
    await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(dialog).toBeHidden();
    await expect(open).toBeFocused();
    expect((await login(original)).status()).toBe(200);

    await open.click();
    await expect(dialog.getByRole("alert")).toHaveCount(0);
    await expect(password).toHaveValue("");
    await password.fill(replacement);
    const response = page.waitForResponse(
      (response) => response.url() === endpoint && response.request().method() === "PUT"
    );
    await password.press("Enter");
    expect((await response).status()).toBe(200);
    await expect(dialog.getByRole("status")).toContainText("Password changed on this IdP.");
    await expect(password).toHaveCount(0);
    expect((await login(original)).status()).toBe(400);
    expect((await login(replacement)).status()).toBe(200);
  } finally {
    expect((await request.delete(`${idp}/users/${sub}`, { headers })).status()).toBe(200);
  }
});
