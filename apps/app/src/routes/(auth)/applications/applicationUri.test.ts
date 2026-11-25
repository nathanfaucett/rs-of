import { describe, expect, it } from "vitest";
import { isApplicationUri } from "./applicationUri";

describe("Application URI input", () => {
  it.each([
    "https://example.test/app%20one?mode=live#section",
    "http://127.0.0.1:5173",
    "urn:example:application",
  ])("accepts %s", (uri) => {
    expect(isApplicationUri(uri)).toBe(true);
  });

  it.each([
    "example.test/app",
    "//example.test",
    "https:///example.test",
    "https://example.test/a b",
    "https://example.test/%zz",
    "https://user:password@example.test",
    "urn:example:",
    "javascript:void(0)",
    "data:text/plain,hello",
    "file:///tmp/app",
  ])("rejects %s", (uri) => {
    expect(isApplicationUri(uri)).toBe(false);
  });
});
