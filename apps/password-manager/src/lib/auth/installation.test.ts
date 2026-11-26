import { afterEach, expect, it, vi } from "vitest";
import { authConfig, NodeSetupRequiredError } from "./installation";

afterEach(() => vi.unstubAllGlobals());

it("returns the configured Node setup link when sign-in is blocked by setup", async () => {
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue(Response.json({ state: "awaiting_choice" })));

  const pending = authConfig(
    {
      PUBLIC_UNIFIED_ORIGIN: "https://unified.example.test",
      PUBLIC_NODE_SETUP_URL: "https://of.example.test/setup",
    },
    "https://password-manager.example.test/callback"
  );

  await expect(pending).rejects.toBeInstanceOf(NodeSetupRequiredError);
  await expect(pending).rejects.toMatchObject({
    message: "Finish Node setup before signing in to sync. You can still use local vaults.",
    setupUrl: "https://of.example.test/setup",
  });
});
