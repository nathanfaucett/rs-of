import { randomUUID } from "node:crypto";
import { type APIRequestContext, expect, type Page, test } from "@playwright/test";
import { appURL, backendURL } from "./backend.mjs";
import { json, passwordSession, signIn } from "./session";

const endpoint = `${backendURL}/idp/applications`;

async function applicationOwner(request: APIRequestContext) {
  const session = await passwordSession(request);
  const token = await json<{ access_token: string }>(
    await request.post(`${backendURL}/idp/oauth2/token`, {
      form: {
        grant_type: "urn:ietf:params:oauth:grant-type:token-exchange",
        subject_token_type: "urn:ietf:params:oauth:token-type:access_token",
        subject_token: session.access_token,
        resource: `${backendURL}/idp`,
      },
    })
  );
  const headers = { Authorization: `Bearer ${token.access_token}` };
  return {
    list: async () =>
      json<{ id: string; name: string; uri: string }[]>(
        await request.get(`${endpoint}?limit=100`, { headers })
      ),
    create: async (name: string, uri: string) =>
      json<{ id: string }>(await request.post(endpoint, { headers, data: { name, uri } })),
    remove: async (id: string) =>
      expect((await request.delete(`${endpoint}/${id}`, { headers })).ok()).toBe(true),
  };
}

async function openApplications(page: Page) {
  await page.goto(`${appURL}/applications`);
  await signIn(page);
  await expect(page).toHaveURL(`${appURL}/applications`);
  await expect(page.getByRole("table")).toBeVisible();
}

test("Application creation rejects unsafe and malformed URIs and names open details", async ({
  page,
  request,
}) => {
  const owner = await applicationOwner(request);
  const name = `Application ${randomUUID()}`;
  const uri = `urn:example:${randomUUID()}`;
  try {
    await openApplications(page);
    const form = page.getByRole("textbox", { name: "URI", exact: true });
    const create = page.getByRole("button", { name: "Create application", exact: true });
    const posted: string[] = [];
    page.on("request", (request) => {
      if (request.url() === endpoint && request.method() === "POST") posted.push(request.url());
    });
    await page.getByRole("textbox", { name: "Name", exact: true }).fill(name);
    for (const invalid of ["not a URI", "https:///missing-authority", "javascript:void(0)"]) {
      await form.fill(invalid);
      await create.click();
      await expect(form).toHaveAttribute("aria-invalid", "true");
      expect(posted).toHaveLength(0);
    }
    await form.fill(uri);
    await create.click();
    const row = page.getByRole("row").filter({ hasText: name });
    const nameLink = row.getByRole("link", { name, exact: true });
    await expect(nameLink).toBeVisible();
    const id = (await owner.list()).find((entry) => entry.uri === uri)?.id;
    if (!id) throw new Error("Created Application is missing from the IdP");
    await expect(nameLink).toHaveAttribute("href", `/applications/${id}`);
    await expect(row.getByRole("link", { name: id, exact: true })).toHaveAttribute(
      "href",
      `/applications/${id}`
    );
    await expect(row.getByRole("cell", { name: uri, exact: true })).toBeVisible();
    await expect(row.getByRole("link", { name: uri, exact: true })).toHaveCount(0);
    await nameLink.click();
    await expect(page).toHaveURL(`${appURL}/applications/${id}`);
    await expect(page.getByRole("textbox", { name: "URI", exact: true })).not.toBeEditable();
    await expect(
      page.getByRole("textbox", { name: "Description", exact: true }).first()
    ).toBeVisible();
    const updated = `${name} updated`;
    await page.getByRole("textbox", { name: "Name", exact: true }).first().fill(updated);
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await expect
      .poll(async () => (await owner.list()).find((entry) => entry.id === id)?.name)
      .toBe(updated);
    await expect(page.getByText("Failed to save application", { exact: true })).toHaveCount(0);
    await expect(page.getByRole("textbox", { name: "URI", exact: true })).toHaveValue(uri);
    page.once("dialog", (dialog) => dialog.accept());
    await page.getByRole("button", { name: "Delete", exact: true }).click();
    await expect(page).toHaveURL(`${appURL}/applications`);
    expect((await owner.list()).some((entry) => entry.id === id)).toBe(false);
  } finally {
    for (const entry of await owner.list()) {
      if (entry.name.startsWith(name)) await owner.remove(entry.id);
    }
  }
});

test("New Client IDs stay copyable without cross-Application detail access", async ({
  page,
  request,
}) => {
  const owner = await applicationOwner(request);
  const application = (await owner.list()).find(({ name }) => name === "password-manager");
  if (!application) throw new Error("Missing password-manager test Application");
  await page.goto(`${appURL}/applications/${application.id}`);
  await signIn(page);
  // The seeded backend is isolated and removes the synthetic registration at teardown.
  await page
    .getByRole("textbox", { name: "Client name", exact: true })
    .fill("Cross-Application Client");
  await page
    .getByRole("textbox", { name: "Redirect URIs", exact: true })
    .fill(`${appURL}/callback`);
  const registration = page.waitForResponse(
    (response) =>
      response.url() === `${backendURL}/idp/oauth2/register` &&
      response.request().method() === "POST"
  );
  await page.getByRole("button", { name: "Create client", exact: true }).click();
  const { client_id } = await (await registration).json();
  expect(client_id).toEqual(expect.any(String));
  const clientRow = page.getByRole("row", { name: /Cross-Application Client/ });
  await expect(clientRow).toContainText(client_id);
  await expect(clientRow.getByRole("link", { name: client_id, exact: true })).toHaveCount(0);
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"], { origin: appURL });
  await clientRow.getByRole("button", { name: "Copy ID" }).click();
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(client_id);
});

test("Application pagination stops at exact and short boundaries and keeps the prior page on failure", async ({
  page,
  request,
}) => {
  const owner = await applicationOwner(request);
  const prefix = `Pagination ${randomUUID()}`;
  const ids: string[] = [];
  try {
    const initialCount = (await owner.list()).length;
    expect(initialCount).toBeLessThan(25);
    for (let index = initialCount; index < 25; index++) {
      const entry = await owner.create(
        `${prefix} ${index}`,
        `https://example.test/${randomUUID()}`
      );
      ids.push(entry.id);
    }
    await openApplications(page);
    const rows = page.getByRole("table").locator("tbody tr");
    const next = page.getByRole("button", { name: "Next", exact: true });
    const prev = page.getByRole("button", { name: "Prev", exact: true });
    const offset = page.getByText("Offset: 0", { exact: true });
    await expect(rows).toHaveCount(25);
    await expect(next).toBeDisabled();

    const search = page.getByRole("searchbox", { name: "Search current page" });
    const sort = page.getByRole("combobox", { name: "Sort current page" });
    await expect(search).toBeVisible();
    await search.fill(prefix.toUpperCase());
    await sort.selectOption("name-desc");
    await expect(rows.first()).toContainText(`${prefix} 24`);

    const last = await owner.create(`${prefix} extra`, `https://example.test/${randomUUID()}`);
    ids.push(last.id);
    await page.reload();
    await expect(next).toBeEnabled();
    await search.fill("nothing matches this query");
    await expect(page.getByText("No matches on this page.")).toBeVisible();
    await expect(next).toBeEnabled();
    await page.route(
      (url) =>
        `${url.origin}${url.pathname}` === endpoint && url.searchParams.get("offset") === "25",
      (route) => route.abort(),
      { times: 1 }
    );
    await next.click();
    await expect(page.getByRole("alert")).toContainText("Could not load applications");
    await expect(offset).toBeVisible();
    await expect(page.getByText("No matches on this page.")).toBeVisible();
    await expect(next).toBeDisabled();
    await search.fill("");
    await expect(rows).toHaveCount(25);
    await search.fill("nothing matches this query");

    await page.getByRole("button", { name: "Retry", exact: true }).click();
    await expect(page.getByRole("alert")).toHaveCount(0);
    await expect(page.getByText("Offset: 25", { exact: true })).toBeVisible();
    await expect(search).toHaveValue("nothing matches this query");
    await expect(page.getByText("No matches on this page.")).toBeVisible();
    await search.fill("");
    await expect(rows).toHaveCount(1);
    await expect(next).toBeDisabled();
    await prev.click();
    await expect(offset).toBeVisible();
    await expect(rows).toHaveCount(25);
    await expect(next).toBeEnabled();
  } finally {
    for (const id of ids) await owner.remove(id);
  }
});
