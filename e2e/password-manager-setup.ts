import { expect, type Page } from "@playwright/test";

const name = "Local during setup";
const masterPassword = "local-only-master-password";

export async function passwordManagerWhileWaiting(page: Page, origin: string, node: string) {
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
  await expect(page).toHaveURL(`${origin}/login`);
  expect(oauth).toEqual([]);
  await page.goto(origin);
  await page.getByRole("button", { name: "Create new vault", exact: true }).click();
  await page.getByLabel("Vault name").fill(name);
  await page.getByLabel("Master password").fill(masterPassword);
  await page.getByLabel("Confirm password").fill(masterPassword);
  await page.getByRole("button", { name: "Create vault", exact: true }).click();
  await expect(page).toHaveURL(`${origin}/vault`);
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
  withLocalVault = false
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
  expect(clientId).toEqual(expect.any(String));
  expect(clientId).not.toBe("password-manager");
  expect(new URLSearchParams(grant.request().postData() ?? "").get("client_id")).toBe(clientId);
  await expect(page).toHaveURL(`${origin}/`);
  await page.reload();
  const show = page.getByRole("button", { name: "Show synced vaults" });
  await expect(show).toBeVisible();
  const [exchange, list] = await Promise.all([
    tokenResponse("urn:ietf:params:oauth:grant-type:token-exchange"),
    page.waitForResponse((res) => res.url() === `${storage}/databases`, { timeout: 20_000 }),
    show.click(),
  ]);
  expect(exchange.ok()).toBe(true);
  expect(new URLSearchParams(exchange.request().postData() ?? "").get("client_id")).toBe(clientId);
  expect(list.ok()).toBe(true);

  if (withLocalVault) {
    await page.getByRole("button", { name: new RegExp(name) }).click();
    await expect(page.getByRole("heading", { name: `Unlock vault: ${name}` })).toBeVisible();
    const passwordInput = page.getByLabel("Master password");
    const unlock = page.getByRole("button", { name: "Unlock", exact: true });
    await passwordInput.fill(password);
    await unlock.click();
    await expect(page.getByRole("alert")).toBeVisible();
    await passwordInput.fill(masterPassword);
    await unlock.click();
    await expect(page).toHaveURL(`${origin}/vault`);
  }
}
