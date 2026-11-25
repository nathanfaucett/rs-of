import { randomUUID } from "node:crypto";
import { expect, type Page, type Request, test } from "@playwright/test";
import { appURL, backendURL } from "./backend.mjs";
import { json, passwordSession, signIn } from "./session";

function isUserCreation(request: Request) {
  return request.method() === "POST" && request.url() === `${backendURL}/idp/users`;
}

async function openCreateUser(page: Page) {
  await page.getByRole("button", { name: "Add user", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Create User", exact: true });
  await expect(dialog).toBeVisible();
  return dialog;
}

test("Create User blocks invalid email and short password before a request", async ({ page }) => {
  const requests: Request[] = [];
  page.on("request", (request) => {
    if (isUserCreation(request)) requests.push(request);
  });
  await page.goto(`${appURL}/users`);
  await signIn(page);
  const dialog = await openCreateUser(page);
  const email = dialog.getByRole("textbox", { name: "Email", exact: true });
  const password = dialog.getByLabel(/^Password/);
  const create = dialog.getByRole("button", { name: "Create", exact: true });
  await dialog.getByRole("textbox", { name: "Name", exact: true }).fill("Validation fixture");
  await email.fill("not-an-email");
  await password.pressSequentially("short");
  await create.click();
  await expect(email).toBeFocused();
  expect(await email.evaluate((input: HTMLInputElement) => input.validationMessage)).not.toBe("");
  await email.fill("validation@example.test");
  await create.click();
  await expect(password).toBeFocused();
  expect(await password.evaluate((input: HTMLInputElement) => input.validationMessage)).not.toBe(
    ""
  );
  expect(requests).toHaveLength(0);
  await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(dialog).toBeHidden();
});

test("Create User submits once on Enter, and the new User can sign in but cannot create Users", async ({
  page,
  browser,
  request,
}) => {
  const session = await passwordSession(request);
  const owner = await json<{ access_token: string }>(
    await request.post(`${backendURL}/idp/oauth2/token`, {
      form: {
        grant_type: "urn:ietf:params:oauth:grant-type:token-exchange",
        subject_token: session.access_token,
        subject_token_type: "urn:ietf:params:oauth:token-type:access_token",
        resource: `${backendURL}/idp`,
      },
    })
  );
  const headers = { Authorization: `Bearer ${owner.access_token}` };
  const email = `create-user-${randomUUID()}@example.test`;
  const password = "synthetic-test-password";
  const submissions: Request[] = [];
  page.on("request", (request) => {
    if (isUserCreation(request)) submissions.push(request);
  });
  await page.goto(`${appURL}/users`);
  await signIn(page);
  const dialog = await openCreateUser(page);
  await dialog.getByRole("textbox", { name: "Name", exact: true }).fill("User creation fixture");
  await dialog.getByRole("textbox", { name: "Email", exact: true }).fill(email);
  const passwordInput = dialog.getByLabel(/^Password/);
  await expect(passwordInput).toHaveAttribute("autocomplete", "new-password");
  await passwordInput.fill(password);
  const restricted = await browser.newContext();
  const created: string[] = [];
  try {
    const [result] = await Promise.all([
      page.waitForResponse((response) => isUserCreation(response.request()), { timeout: 10000 }),
      passwordInput.press("Enter"),
    ]);
    if (result.ok()) created.push((await result.json()).sub);
    expect(result.status()).toBe(200);
    await expect(dialog).toBeHidden();
    await expect(page.getByRole("cell", { name: email, exact: true })).toBeVisible();
    expect(submissions).toHaveLength(1);

    const search = page.getByRole("searchbox", { name: "Search current page" });
    await search.fill("no matching user");
    await expect(page.getByText("No matches on this page.")).toBeVisible();
    await search.fill(email.toUpperCase());
    const user = page.getByRole("link", { name: "User creation fixture", exact: true });
    await expect(user).toHaveAttribute("href", `/users/${created[0]}`);
    await user.press("Enter");
    await expect(page).toHaveURL(`${appURL}/users/${created[0]}`);

    const restrictedPage = await restricted.newPage();
    await restrictedPage.goto(`${appURL}/users`);
    await signIn(restrictedPage, email, password);
    await expect(restrictedPage).toHaveURL(`${appURL}/users`);
    const denied = await openCreateUser(restrictedPage);
    const deniedName = denied.getByRole("textbox", { name: "Name", exact: true });
    const deniedEmail = denied.getByRole("textbox", { name: "Email", exact: true });
    const deniedPassword = denied.getByLabel(/^Password/);
    await deniedName.fill("Unassigned User fixture");
    await deniedEmail.fill(`denied-${email}`);
    await deniedPassword.fill(password);
    const [denial] = await Promise.all([
      restrictedPage.waitForResponse((response) => isUserCreation(response.request())),
      deniedPassword.press("Enter"),
    ]);
    if (denial.ok()) created.push((await denial.json()).sub);
    expect(denial.status()).toBe(403);
    await expect(denied.getByRole("alert")).toContainText(
      "You do not have permission to create users."
    );
    await expect(deniedName).toHaveValue("Unassigned User fixture");
    await expect(deniedEmail).toHaveValue(`denied-${email}`);
    await expect(deniedPassword).toHaveValue(password);
  } finally {
    await restricted.close();
    for (const id of created) {
      expect((await request.delete(`${backendURL}/idp/users/${id}`, { headers })).status()).toBe(
        200
      );
    }
  }
});
