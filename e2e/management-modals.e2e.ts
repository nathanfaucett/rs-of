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
