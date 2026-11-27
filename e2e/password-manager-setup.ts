import { expect, type Page } from "@playwright/test";

export const localVault = "Local during setup";
export const masterPassword = "local-only-master-password";
export const localRecord = "Record saved before Node setup";
export const localSecret = "A local record must survive setup.";

export async function passwordManagerWhileWaiting(
  page: Page,
  origin: string,
  node: string,
  app: string
) {
  const oauth: string[] = [];
  await page.goto(origin);
  await page.getByRole("link", { name: "Sign in to sync" }).click();
  page.on("request", (request) => {
    const { pathname } = new URL(request.url());
    if (pathname.startsWith("/idp/")) oauth.push(pathname);
  });
  const [descriptor] = await Promise.all([
    page.waitForResponse(`${node}/setup/node`, { timeout: 10_000 }),
    page.getByRole("button", { name: "Sign in with IdP" }).click(),
  ]);
  expect(await descriptor.json()).toEqual({ state: "awaiting_choice" });
  await expect(page.getByRole("alert")).toContainText("Finish Node setup");
  await expect(page.getByRole("link", { name: "Open Node setup", exact: true })).toHaveAttribute(
    "href",
    `${app}/setup`
  );
  await expect(page).toHaveURL(`${origin}/login`);
  expect(oauth).toEqual([]);
  const blockedNode = `${node}/**`;
  await page.route(blockedNode, (route) => route.abort());
  await page.goto(origin);
  await page.getByRole("button", { name: "Create new vault", exact: true }).click();
  await page.getByLabel("Vault name").fill(localVault);
  await page.getByLabel("Master password", { exact: true }).fill(masterPassword);
  await page.getByLabel("Confirm password").fill(masterPassword);
  await page.getByRole("button", { name: "Create vault", exact: true }).click();
  await expect(page).toHaveURL(`${origin}/vault`);
  await page.getByRole("button", { name: "New item", exact: true }).click();
  await page.getByRole("menuitem", { name: "New secure note" }).click();
  await page.getByRole("textbox", { name: "Name", exact: true }).fill(localRecord);
  await page.getByLabel("Notes", { exact: true }).fill(localSecret);
  await page.getByRole("button", { name: "Create item" }).click();
  await expect(page).toHaveURL(`${origin}/vault`);
  await expect(page.getByText(localRecord, { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Lock vault" }).click();
  const vault = page.getByRole("button", { name: new RegExp(localVault) });
  await expect(vault).toBeVisible();
  const vaultId = (await vault.innerText()).split("ID: ")[1]?.trim();
  expect(vaultId).toEqual(expect.stringMatching(/^[a-f0-9-]+$/));
  await page.unroute(blockedNode);
  return vaultId;
}

export async function passwordManagerSignin(
  page: Page,
  {
    origin,
    idp,
    storage,
    username,
    password,
  }: Record<"origin" | "idp" | "storage" | "username" | "password", string>,
  withLocalVault = false,
  checkLegacySelector = true
) {
  await page.goto(origin);
  await page.getByRole("link", { name: "Sign in to sync" }).click();
  await page.getByRole("button", { name: "Sign in with IdP" }).click();
  const login = page.getByRole("textbox", { name: "Username", exact: true });
  await expect(login)
    .toBeVisible({ timeout: 6_000 })
    .catch(async (cause) => {
      throw new Error(
        `Expected Installation sign-in at ${new URL(page.url()).pathname}: ${await page.locator("body").innerText()}`,
        { cause }
      );
    });
  await login.fill(username);
  await page.getByLabel("Password", { exact: true }).fill(password);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await expect(page).toHaveURL(/\/authorize\?/);
  const params = new URL(page.url()).searchParams;
  expect(params.get("application_id")).toBe(origin);
  expect(params.has("client_id")).toBe(false);
  expect(params.get("code_challenge_method")).toBe("S256");

  const tokenResponse = (grant: string) =>
    page.waitForResponse(
      (res) =>
        res.url() === `${idp}/oauth2/token` &&
        new URLSearchParams(res.request().postData() ?? "").get("grant_type") === grant,
      { timeout: 20_000 }
    );
  const [registration] = await Promise.all([
    page.waitForResponse(
      (res) => res.url() === `${idp}/oauth2/register` && res.request().method() === "POST",
      { timeout: 15_000 }
    ),
    page.getByRole("button", { name: "Accept", exact: true }).click({ timeout: 10_000 }),
  ]);
  expect(registration.ok()).toBe(true);
  const { client_id: clientId } = await registration.json();
  const [grant] = await Promise.all([
    tokenResponse("authorization_code"),
    page.getByRole("button", { name: "Allow", exact: true }).click({ timeout: 10_000 }),
  ]);
  expect(grant.ok()).toBe(true);
  const { access_token: accessToken } = (await grant.json()) as { access_token: string };
  expect(clientId).toEqual(expect.any(String));
  expect(clientId).not.toBe("password-manager");
  expect(new URLSearchParams(grant.request().postData() ?? "").get("client_id")).toBe(clientId);
  await expect(page).toHaveURL(`${origin}/`);
  await page.reload();
  if (checkLegacySelector) {
    const show = page.getByRole("button", { name: "Show synced vaults" });
    await expect(show).toBeVisible();
    const [exchange, list] = await Promise.all([
      tokenResponse("urn:ietf:params:oauth:grant-type:token-exchange"),
      page.waitForResponse((res) => res.url() === `${storage}/databases`, { timeout: 20_000 }),
      show.click(),
    ]);
    expect(exchange.ok()).toBe(true);
    expect(new URLSearchParams(exchange.request().postData() ?? "").get("client_id")).toBe(
      clientId
    );
    expect(list.ok()).toBe(true);
  }

  if (withLocalVault) {
    await page.getByRole("button", { name: new RegExp(localVault) }).click();
    await expect(page.getByRole("heading", { name: `Unlock vault: ${localVault}` })).toBeVisible();
    const passwordInput = page.getByLabel("Master password", { exact: true });
    const unlock = page.getByRole("button", { name: "Unlock", exact: true });
    await passwordInput.fill(password);
    await unlock.click();
    await expect(page.getByRole("alert")).toBeVisible();
    await passwordInput.fill(masterPassword);
    await unlock.click();
    await expect(page).toHaveURL(`${origin}/vault`);
    await expect(page.getByText(localRecord, { exact: true })).toBeVisible();
  }
  return { clientId: clientId as string, accessToken };
}

export async function passwordManagerOpensLegacyVault(
  page: Page,
  origin: string,
  resourceId: string
) {
  const name = "Legacy synced vault";
  await page.evaluate(
    async ({ name, masterPassword, resourceId }) => {
      const fixture = "/src/test/legacy-browser-fixture.ts";
      const { createLegacyVault } = await import(fixture);
      await createLegacyVault(resourceId, masterPassword, name);
    },
    { name, masterPassword, resourceId }
  );
  await page.reload();
  await page.getByRole("button", { name: "Show synced vaults" }).click();
  await page.getByRole("button", { name: new RegExp(name) }).click();
  await expect(page.getByLabel("Master password", { exact: true })).toBeFocused();
  await page.getByLabel("Master password", { exact: true }).fill(masterPassword);
  await page.getByRole("button", { name: "Unlock", exact: true }).click();
  await expect(page).toHaveURL(`${origin}/vault`);
}
