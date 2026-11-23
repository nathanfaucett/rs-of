import { expect } from "vitest";

const { VITE_E2E_BACKEND_URL: backendURL } = import.meta.env;

export { backendURL };

export async function json<T = unknown>(response: Response): Promise<T> {
  expect(response.ok, `${response.url}: ${response.status} ${await response.clone().text()}`).toBe(
    true
  );
  return (await response.json()) as T;
}

export async function session() {
  return json<{ access_token: string }>(
    await fetch(`${backendURL}/idp/oauth2/token`, {
      method: "POST",
      body: new URLSearchParams({
        grant_type: "password",
        client_id: "local-login",
        username: "admin@local.test",
        password: "password",
        scope: "openid profile email",
      }),
    })
  );
}
