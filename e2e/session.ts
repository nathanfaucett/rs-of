import { createHash, randomBytes } from "node:crypto";
import { expect, type APIRequestContext, type Page } from "@playwright/test";
import { StorageClient } from "../packages/storage-client/src/index";
import { backendURL } from "./backend.mjs";
import { checkedFetch } from "./http";

export async function json(response: Awaited<ReturnType<APIRequestContext["post"]>>) {
  expect(
    response.ok(),
    `${response.url()}: ${response.status()} ${await response.text()}`
  ).toBeTruthy();
  return (await response.json()) as Record<string, string>;
}

export async function passwordSession(request: APIRequestContext) {
  return json(
    await request.post(`${backendURL}/idp/oauth2/token`, {
      form: {
        grant_type: "password",
        client_id: "local-login",
        username: "admin@local.test",
        password: "password",
        scope: "openid profile email",
      },
    })
  );
}

export async function frontendSession(
  request: APIRequestContext,
  clientId: string,
  origin: string
) {
  const login = await passwordSession(request);
  const headers = { Authorization: `Bearer ${login.access_token}` };
  const redirect_uri = `${origin}/callback`;
  const scope = "openid profile email";
  await json(
    await request.post(`${backendURL}/idp/oauth2/auth/approve-for-user`, {
      headers,
      data: { client_id: clientId, redirect_uri, scope },
    })
  );
  const verifier = randomBytes(32).toString("base64url");
  const state = randomBytes(16).toString("hex");
  const authorization = await json(
    await request.post(`${backendURL}/idp/oauth2/auth`, {
      headers,
      data: {
        response_type: "code",
        client_id: clientId,
        redirect_uri,
        scope,
        state,
        code_challenge: createHash("sha256").update(verifier).digest("base64url"),
        code_challenge_method: "S256",
      },
    })
  );
  expect(authorization.state).toBe(state);
  const session = await json(
    await request.post(`${backendURL}/idp/oauth2/token`, {
      form: {
        grant_type: "authorization_code",
        client_id: clientId,
        redirect_uri,
        code: authorization.code,
        code_verifier: verifier,
      },
    })
  );
  expect(session.access_token).toEqual(expect.any(String));
  expect(session.id_token).toEqual(expect.any(String));
  const user = await json(
    await request.get(`${backendURL}/idp/userinfo`, {
      headers: { Authorization: `Bearer ${session.access_token}` },
    })
  );
  expect(user.email).toBe("admin@local.test");
  return session;
}

export function storageClient(clientId: string, accessToken: string) {
  return new StorageClient({
    idpBaseUrl: `${backendURL}/idp`,
    storageBaseUrl: `${backendURL}/storage`,
    clientId,
    bearerToken: () => accessToken,
    fetch: checkedFetch,
  });
}

export async function signIn(page: Page) {
  const username = page.getByRole("textbox", { name: "Username", exact: true });
  await expect(username, "Sign-in must be available without installation setup").toBeVisible();
  await username.fill("admin@local.test");
  await page.getByLabel("Password", { exact: true }).fill("password");
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
}
