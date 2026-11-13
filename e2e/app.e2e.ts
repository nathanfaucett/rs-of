import { expect, test } from "@playwright/test";
import { appURL, backendURL } from "./backend.mjs";
import { signIn, storageClient } from "./session";

test("app signs in and uses its session for Device and Storage access", async ({
  page,
  request,
}) => {
  await page.goto(`${appURL}/signin`);
  const tokenResponse = page.waitForResponse(
    (response) =>
      response.url() === `${backendURL}/idp/oauth2/token` &&
      response.request().postData()?.includes("grant_type=password") === true
  );
  await signIn(page);
  const response = await tokenResponse;
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
    headers: { Authorization: `Bearer ${(await resourceToken.json()).access_token}` },
  });
  expect(devices.ok()).toBeTruthy();
  expect(await devices.json()).toEqual(expect.any(Array));
  const storage = storageClient("local-login", token.access_token);
  expect(await storage.listDatabases()).toEqual(expect.any(Array));
  await page.reload();
  await expect(page.getByRole("heading", { name: /devices/i }).first()).toBeVisible();
});
