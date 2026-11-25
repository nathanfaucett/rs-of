import { expect, test, vi } from "vitest";
import { requireSessionAccessToken, SessionExpiredError } from "./sessionAccess";

function session() {
  let stored = { access_token: "original", refresh_token: "refresh" };
  const renewal = Promise.withResolvers<string>();
  const client = {
    getStoredTokenResponse: () => stored,
    getAccessToken: () => renewal.promise,
  };
  const replace = (access_token: string) => {
    stored = { access_token, refresh_token: "replacement" };
  };
  const clear = vi.fn(() => {
    stored = { access_token: "", refresh_token: "" };
  });
  return {
    renewal,
    replace,
    clear,
    access: (current = () => true) => requireSessionAccessToken(client, current, clear),
  };
}

for (const outcome of ["success", "failure"]) {
  test(`a stale renewal ${outcome} cannot use or clear a replacement session`, async () => {
    const { access, clear, renewal, replace } = session();
    const pending = access();
    replace("new session");
    if (outcome === "success") renewal.resolve("stale access");
    else renewal.reject(new Error("Refresh failed"));
    await expect(pending).rejects.toThrow("Sign-in session changed");
    expect(clear).not.toHaveBeenCalled();
  });
}

test("a failure for a superseded client cannot clear the active session", async () => {
  const { access, clear, renewal } = session();
  let current = true;
  const pending = access(() => current);
  current = false;
  renewal.reject(new Error("Refresh failed"));
  await expect(pending).rejects.toThrow("Sign-in session changed");
  expect(clear).not.toHaveBeenCalled();
});

test("a shared failed renewal expires only the original session once", async () => {
  const { access, clear, renewal } = session();
  const first = access();
  const second = access();
  renewal.reject(new Error("Refresh failed"));
  await expect(first).rejects.toBeInstanceOf(SessionExpiredError);
  await expect(second).rejects.toThrow("Sign-in session changed");
  expect(clear).toHaveBeenCalledTimes(1);
});
