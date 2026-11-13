import { expect, test } from "@playwright/test";
import { appURL, backendURL } from "./backend.mjs";
import { json, signIn, storageClient } from "./session";

test("browser sign-in bypasses installation setup and retains its protected session on reload", async ({
  page,
}) => {
  await page.goto(`${appURL}/devices`);
  await expect(page).toHaveURL(/\/signin/);
  await signIn(page);
  await expect(page).toHaveURL(`${appURL}/devices`);
  await expect(
    page.getByRole("heading", { name: /devices/i }).first(),
  ).toBeVisible();
  await page.reload();
  await expect(page).toHaveURL(`${appURL}/devices`);
  await expect(
    page.getByRole("heading", { name: /devices/i }).first(),
  ).toBeVisible();
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
  await expect(
    page.getByRole("heading", { name: "Invalid Request", exact: true }),
  ).toBeVisible();
  await expect(page.getByRole("alert")).toContainText(
    "New clients require application_id and registration metadata",
  );

  url.searchParams.set("application_id", "https://other.test");
  url.searchParams.set(
    "registration",
    JSON.stringify({
      application: { uri: "https://app.test" },
      clientName: "Untrusted client",
      profile: "native_application",
      tokenEndpointAuthMethod: "none",
    }),
  );
  await page.goto(url.href);
  await expect(page.getByRole("alert")).toContainText(
    "application_id must match registration.application.uri",
  );

  url.searchParams.delete("application_id");
  url.searchParams.delete("registration");
  url.searchParams.set("client_id", "unknown");
  await page.goto(url.href);
  await expect(
    page.getByText("The client ID you provided does not exist.", {
      exact: false,
    }),
  ).toBeVisible();
  await expect(
    page.getByText("These details were supplied by the application", {
      exact: false,
    }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: /accept|register/i }),
  ).toHaveCount(0);
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
        response.request().postData()?.includes("grant_type=password") === true,
    ),
    signIn(page),
  ]);
  expect(response.ok()).toBeTruthy();
  const token = await response.json();
  await expect(page).not.toHaveURL(/\/signin/);
  await page.goto(`${appURL}/devices`);
  await expect(
    page.getByRole("heading", { name: /devices/i }).first(),
  ).toBeVisible();
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
  await expect(
    page.getByRole("heading", { name: /devices/i }).first(),
  ).toBeVisible();
});
