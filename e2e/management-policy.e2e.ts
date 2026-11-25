import { expect, test } from "@playwright/test";
import { appURL, backendURL } from "./backend.mjs";
import { frontendSession, json, passwordSession, signIn } from "./session";

test("Management Roles and Permissions work outside the sign-in Application", async ({
  browser,
  page,
  request,
}) => {
  const login = await passwordSession(request);
  const exchange = async (resource: string) => {
    const token = await json<{ access_token: string }>(
      await request.post(`${backendURL}/idp/oauth2/token`, {
        form: {
          grant_type: "urn:ietf:params:oauth:grant-type:token-exchange",
          subject_token: login.access_token,
          subject_token_type: "urn:ietf:params:oauth:token-type:access_token",
          resource,
        },
      })
    );
    return { Authorization: `Bearer ${token.access_token}` };
  };
  const idp = await exchange(`${backendURL}/idp`);
  const management = await exchange("idp-management");
  const user = await json<{ sub: string }>(
    await request.get(`${backendURL}/idp/userinfo`, {
      headers: { Authorization: `Bearer ${login.access_token}` },
    })
  );
  const app = await json<{ id: string }>(
    await request.post(`${backendURL}/idp/applications`, {
      headers: idp,
      data: { name: "Role test", uri: "https://role-test.example" },
    })
  );
  const appPolicy = `${backendURL}/management/applications/${app.id}`;
  let roleId: string | undefined;
  let permissionId: string | undefined;
  try {
    await page.goto(`${appURL}/applications/${app.id}`);
    await signIn(page);
    await expect(page.getByText("No roles found.")).toBeVisible();
    await expect(page.getByText("No permissions found.")).toBeVisible();

    const roleForm = page.locator("form").filter({
      has: page.getByRole("button", { name: "Create role", exact: true }),
    });
    const roleName = roleForm.getByRole("textbox", { name: "Name", exact: true });
    await roleName.fill("Vault reader");
    const policyLoads: string[] = [];
    page.on("request", (request) => {
      if (request.method() === "GET" && request.url().startsWith(appPolicy)) {
        policyLoads.push(request.url());
      }
    });
    await page
      .getByRole("region", { name: "Application", exact: true })
      .getByLabel("Description")
      .fill("Policy test draft");
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await expect(page.getByText("Application saved", { exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Save", exact: true })).toBeEnabled();
    await expect(roleName).toHaveValue("Vault reader");
    expect(policyLoads).toEqual([]);
    const createdRole = page.waitForResponse(
      (response) =>
        response.url() === `${appPolicy}/roles` && response.request().method() === "POST"
    );
    await roleForm.getByRole("button", { name: "Create role" }).click();
    const roleResponse = await createdRole;
    expect(roleResponse.ok(), await roleResponse.text()).toBeTruthy();
    roleId = (await roleResponse.json()).id;
    await expect(page.getByRole("cell", { name: "Vault reader", exact: true })).toBeVisible();

    const permissionForm = page.locator("form").filter({
      has: page.getByRole("button", { name: "Create permission", exact: true }),
    });
    await permissionForm
      .getByRole("textbox", { name: "Name", exact: true })
      .fill("vault.read.test");
    const createdPermission = page.waitForResponse(
      (response) =>
        response.url() === `${appPolicy}/permissions` && response.request().method() === "POST"
    );
    await permissionForm.getByRole("button", { name: "Create permission" }).click();
    const permissionResponse = await createdPermission;
    expect(permissionResponse.ok(), await permissionResponse.text()).toBeTruthy();
    permissionId = (await permissionResponse.json()).id;
    await page.getByRole("combobox", { name: "Permission", exact: true }).selectOption({
      label: "vault.read.test",
    });
    await page.getByRole("button", { name: "Assign permission", exact: true }).click();
    await expect(
      page
        .getByRole("row")
        .filter({ hasText: "vault.read.test" })
        .filter({
          has: page.getByRole("button", { name: "Revoke", exact: true }),
        })
    ).toBeVisible();

    await roleName.fill("Vault reader");
    const description = roleForm.getByLabel("Description", { exact: true });
    await description.fill("Keep this draft");
    await roleForm.getByRole("button", { name: "Create role" }).click();
    await expect(
      page.getByRole("region", { name: "Roles", exact: true }).getByRole("alert")
    ).toContainText("Cannot create a role");
    await expect(roleName).toHaveValue("Vault reader");
    await expect(description).toHaveValue("Keep this draft");

    const assignment = await request.post(`${appPolicy}/users/${user.sub}/roles/${roleId}`, {
      headers: management,
    });
    expect(assignment.ok()).toBeTruthy();
    await page.goto(`${appURL}/users/${user.sub}`);
    await expect(page.getByRole("cell", { name: "Vault reader", exact: true })).toBeVisible();

    // Use a real session for the same User from a non-operator OAuth Client.
    const restricted = await frontendSession(request, "app", appURL);
    const storageState = await page.context().storageState();
    const oidcResponse = storageState.origins
      .find(({ origin }) => origin === appURL)
      ?.localStorage.find(
        ({ name }) => name.startsWith("oidc.token:") && name.endsWith(":response")
      );
    if (!oidcResponse) throw new Error("Operator login did not persist an OIDC response");
    oidcResponse.value = JSON.stringify({
      ...restricted,
      access_token_expires_at: Date.now() + 120_000,
    });
    const restrictedPage = await browser.newPage({ storageState });
    try {
      await restrictedPage.goto(`${appURL}/applications/${app.id}`);
      await expect(
        restrictedPage.getByRole("region", { name: "Roles", exact: true }).getByRole("alert")
      ).toContainText("You do not have permission to list roles.");
      await expect(
        restrictedPage.getByRole("region", { name: "Permissions", exact: true }).getByRole("alert")
      ).toContainText("You do not have permission to list permissions.");
      await expect(
        restrictedPage.getByRole("button", { name: "Create role", exact: true })
      ).toHaveCount(0);
      await expect(
        restrictedPage.getByRole("button", { name: "Create permission", exact: true })
      ).toHaveCount(0);
    } finally {
      await restrictedPage.context().close();
    }
  } finally {
    if (roleId) {
      await request.delete(`${appPolicy}/users/${user.sub}/roles/${roleId}`, {
        headers: management,
      });
      await request.delete(`${appPolicy}/roles/${roleId}`, { headers: management });
    }
    if (permissionId) {
      await request.delete(`${appPolicy}/permissions/${permissionId}`, { headers: management });
    }
    await request.delete(`${backendURL}/idp/applications/${app.id}`, { headers: idp });
  }
});
