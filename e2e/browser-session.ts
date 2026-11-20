import { expect } from "vitest";

export const backendURL = import.meta.env.VITE_E2E_BACKEND_URL;

export async function json(response: Response) {
  expect(
    response.ok,
    `${response.url}: ${response.status} ${await response.clone().text()}`,
  ).toBe(true);
  return response.json();
}

export async function session() {
  return json(
    await fetch(`${backendURL}/idp/oauth2/token`, {
      method: "POST",
      body: new URLSearchParams({
        grant_type: "password",
        client_id: "local-login",
        username: "admin@local.test",
        password: "password",
        scope: "openid profile email",
      }),
    }),
  );
}
