import { expect, test } from "@playwright/test";
import { appURL, backendURL } from "./backend.mjs";
import { frontendSession, json, signIn } from "./session";

test("Management Roles and Permissions work outside the sign-in Application", async ({
  browser,
  page,
  request,
}) => {
  const marker = "management.policy.administer";
  type ScopedToken = {
    access_token: string;
    scope: string;
    id_token: string;
    refresh_token: string;
  };
  const login = await json<ScopedToken>(
    await request.post(`${backendURL}/idp/oauth2/token`, {
      form: {
        grant_type: "password",
        client_id: "local-login",
        username: "admin@local.test",
        password: "password",
        scope: `openid ${marker}`,
        resource: "idp-management",
      },
    })
  );
  expect(login.scope).toBe("openid");
  const tokenScopes = (jwt: string) => {
    const { scope }: { scope: string } = JSON.parse(
      Buffer.from(jwt.split(".")[1] ?? "", "base64url").toString()
    );
    return scope ? scope.split(" ").map((item) => JSON.parse(item) as string) : [];
  };
  for (const jwt of [login.access_token, login.id_token, login.refresh_token]) {
    expect(tokenScopes(jwt)).toEqual(["openid"]);
  }
  const exchange = async (resource: string, subject = login.access_token) => {
    const token = await json<ScopedToken>(
      await request.post(`${backendURL}/idp/oauth2/token`, {
        form: {
          grant_type: "urn:ietf:params:oauth:grant-type:token-exchange",
          subject_token: subject,
          subject_token_type: "urn:ietf:params:oauth:token-type:access_token",
          resource,
        },
      })
    );
    expect(token.scope?.split(" ").includes(marker)).toBe(resource === "idp-management");
    expect(tokenScopes(token.access_token).includes(marker)).toBe(resource === "idp-management");
    for (const jwt of [token.id_token, token.refresh_token]) {
      expect(tokenScopes(jwt)).not.toContain(marker);
    }
    return { Authorization: `Bearer ${token.access_token}` };
  };
  const idp = await exchange(`${backendURL}/idp`);
  const management = await exchange("idp-management");
  await exchange(`${backendURL}/idp`, management.Authorization.slice("Bearer ".length));
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

    const revoke = page.getByRole("button", { name: "Revoke", exact: true });
    await page.route(
      `${appPolicy}/roles/${roleId}/permissions/${permissionId}`,
      (route) => route.fulfill({ status: 403, headers: { "access-control-allow-origin": appURL } }),
      { times: 1 }
    );
    await revoke.click();
    await expect(page.getByRole("alert").filter({ hasText: "revoke a permission" })).toBeVisible();
    await expect(permissionForm.getByRole("button", { name: "Create permission" })).toBeEnabled();

    await roleName.fill("Vault reader");
    const description = roleForm.getByLabel("Description", { exact: true });
    await description.fill("Keep this draft");
    await roleForm.getByRole("button", { name: "Create role" }).click();
    await expect(
      page.getByRole("region", { name: "Roles", exact: true }).getByRole("alert")
    ).toContainText("Cannot create a role");
    await expect(roleName).toHaveValue("Vault reader");
    await expect(description).toHaveValue("Keep this draft");

    await page.goto(`${appURL}/users/${user.sub}`);
    await page
      .getByRole("combobox", { name: "Application", exact: true })
      .selectOption({ label: "Role test" });
    await page
      .getByRole("combobox", { name: "Role", exact: true })
      .selectOption({ label: "Vault reader" });
    await page.getByRole("button", { name: "Assign role", exact: true }).click();
    const assignedRole = page.getByRole("row").filter({
      has: page.getByRole("cell", { name: "Vault reader", exact: true }),
    });
    await expect(assignedRole).toBeVisible();
    page.once("dialog", (dialog) => dialog.accept());
    await assignedRole.getByRole("button", { name: "Revoke", exact: true }).click();
    await expect(assignedRole).toHaveCount(0);

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
