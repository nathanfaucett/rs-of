import { mount, unmount } from "svelte";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { page } from "vitest/browser";
import LockSettingsControl from "$lib/vault/LockSettings.svelte";
import type { LockSettings } from "$lib/vault/lock";
import { vaultStateModule } from "./vault.svelte";

vi.mock("$app/navigation", () => ({ goto: vi.fn().mockResolvedValue(undefined) }));
vi.mock("$env/dynamic/public", () => ({ env: {} }));

const password = "synthetic-lock-settings-password";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

function unlockedVault() {
  const data = vaultStateModule.vaultData;
  if (!data) throw new Error("Expected an unlocked vault");
  return data;
}

beforeEach(async () => {
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(0);
  await vaultStateModule.createVault(password, "Test vault");
});

afterEach(async () => {
  vi.restoreAllMocks();
  await vaultStateModule.lockVault();
  vi.useRealTimers();
});

test("applies the confirmed timeout and starts its deadline at confirmation", async () => {
  const { vault, inactivity } = unlockedVault();
  const confirmation = deferred<LockSettings>();
  vi.spyOn(vault, "saveLockSettings").mockReturnValue(confirmation.promise);

  const save = vaultStateModule.saveLockSettings({ timeoutMinutes: 15 });
  expect(unlockedVault().lockSettings.timeoutMinutes).toBe(5);
  vi.setSystemTime(30_000);
  confirmation.resolve({ timeoutMinutes: 1 });
  await expect(save).resolves.toBe(true);
  expect(unlockedVault().lockSettings.timeoutMinutes).toBe(1);

  vi.setSystemTime(89_999);
  expect(inactivity.check()).toBe(false);
  vi.setSystemTime(90_000);
  expect(inactivity.check()).toBe(true);
});

test("a failed save keeps the displayed and active timeout without showing success", async () => {
  const { vault, inactivity, lockSettings } = unlockedVault();
  vi.spyOn(vault, "saveLockSettings").mockRejectedValue(new Error("Storage unavailable"));
  const control = mount(LockSettingsControl, {
    target: document.body,
    props: { settings: lockSettings, onsave: vaultStateModule.saveLockSettings },
  });
  try {
    const timeout = page.getByLabelText("Lock after inactivity");
    vi.setSystemTime(30_000);
    await timeout.selectOptions("1");
    await expect.element(timeout).toHaveValue("1");
    await page.getByRole("button", { name: "Save lock settings" }).click();
    await expect
      .element(page.getByRole("alert"))
      .toHaveTextContent("previous timeout is still active");
    await expect.element(timeout).toHaveValue("5");
    await expect.element(page.getByRole("status")).not.toBeInTheDocument();
  } finally {
    await unmount(control);
  }
  expect(unlockedVault().lockSettings.timeoutMinutes).toBe(5);
  vi.setSystemTime(299_999);
  expect(inactivity.check()).toBe(false);
  vi.setSystemTime(300_000);
  expect(inactivity.check()).toBe(true);
});

test("a save cannot apply while the old vault is closing", async () => {
  const { vault, inactivity } = unlockedVault();
  const confirmation = deferred<LockSettings>();
  const close = deferred<void>();
  vi.spyOn(vault, "saveLockSettings").mockReturnValue(confirmation.promise);
  vi.spyOn(vault, "close").mockReturnValue(close.promise);

  const save = vaultStateModule.saveLockSettings({ timeoutMinutes: 1 });
  const locking = vaultStateModule.lockVault();
  confirmation.resolve({ timeoutMinutes: 1 });
  const applied = await save;
  close.resolve(undefined);
  await locking;
  expect(applied).toBe(false);
  expect(vaultStateModule.vaultData).toBeNull();
  vi.setSystemTime(60_000);
  expect(inactivity.check()).toBe(false);
});

test("a save from a closed session cannot apply after unlocking again", async () => {
  const { vault } = unlockedVault();
  const confirmation = deferred<LockSettings>();
  vi.spyOn(vault, "saveLockSettings").mockReturnValue(confirmation.promise);

  const save = vaultStateModule.saveLockSettings({ timeoutMinutes: 1 });
  await vaultStateModule.lockVault();
  await vaultStateModule.unlockExistingVault(password, {
    id: vault.id,
    name: "Test vault",
    status: "ready",
    migrated: false,
  });
  confirmation.resolve({ timeoutMinutes: 1 });
  await expect(save).resolves.toBe(false);
  expect(unlockedVault().lockSettings.timeoutMinutes).toBe(5);
  vi.setSystemTime(60_000);
  expect(unlockedVault().inactivity.check()).toBe(false);
});

test("returning to the selector closes the vault and stops its deadline", async () => {
  const { vault, inactivity } = unlockedVault();
  const close = vi.spyOn(vault, "close");

  await vaultStateModule.showVaultSelector();

  expect(vaultStateModule.state.status).toBe("select-vault");
  expect(close).toHaveBeenCalledOnce();
  vi.setSystemTime(300_001);
  expect(inactivity.check()).toBe(false);
});
