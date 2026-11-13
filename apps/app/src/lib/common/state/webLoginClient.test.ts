import { expect, test } from "vitest";
import { configuredWebLoginClient } from "./webLoginClient";

const env = {
  PUBLIC_IDP_BASE_URL: "http://127.0.0.1:1355/idp",
  PUBLIC_OIDC_CLIENT_ID: "local-login",
};

test("uses the explicitly configured browser login client only for the selected IdP", () => {
  expect(configuredWebLoginClient(env, env.PUBLIC_IDP_BASE_URL, false)).toBe(
    "local-login",
  );
  expect(
    configuredWebLoginClient(env, `${env.PUBLIC_IDP_BASE_URL}/`, false),
  ).toBe("local-login");
  expect(
    configuredWebLoginClient(env, "https://production.example/idp", false),
  ).toBeUndefined();
});

test.each([
  "http://production.example/idp",
  "ftp://localhost/idp",
  "https://user:secret@production.example/idp",
  "https://production.example/idp?issuer=other",
  "https://production.example/idp#issuer",
  "not-a-url",
])("rejects an unsafe configured browser issuer: %s", (issuer) => {
  expect(
    configuredWebLoginClient(
      { ...env, PUBLIC_IDP_BASE_URL: issuer },
      issuer,
      false,
    ),
  ).toBeUndefined();
});

test("supports an explicitly configured HTTPS browser client without a seeded fallback", () => {
  const issuer = "https://production.example/idp";
  expect(
    configuredWebLoginClient(
      { PUBLIC_IDP_BASE_URL: issuer, PUBLIC_OIDC_CLIENT_ID: "company-login" },
      issuer,
      false,
    ),
  ).toBe("company-login");
});

test("never changes independent desktop login, including remote desktop selection", () => {
  expect(
    configuredWebLoginClient(env, env.PUBLIC_IDP_BASE_URL, true),
  ).toBeUndefined();
  expect(
    configuredWebLoginClient(
      { ...env, PUBLIC_IDP_BASE_URL: "https://production.example/idp" },
      "https://production.example/idp",
      true,
    ),
  ).toBeUndefined();
});

test("does not infer a seeded server or client when browser configuration is absent", () => {
  expect(
    configuredWebLoginClient({}, env.PUBLIC_IDP_BASE_URL, false),
  ).toBeUndefined();
  expect(
    configuredWebLoginClient(
      { PUBLIC_IDP_BASE_URL: env.PUBLIC_IDP_BASE_URL },
      env.PUBLIC_IDP_BASE_URL,
      false,
    ),
  ).toBeUndefined();
  expect(
    configuredWebLoginClient(
      { PUBLIC_OIDC_CLIENT_ID: "local-login" },
      env.PUBLIC_IDP_BASE_URL,
      false,
    ),
  ).toBeUndefined();
});
