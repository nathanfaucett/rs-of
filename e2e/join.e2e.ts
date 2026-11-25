import { type ChildProcess, spawn } from "node:child_process";
import { once } from "node:events";
import { expect, test } from "@playwright/test";

function setting(name: string): string {
  const value = process.env[`JOIN_TEST_${name}`];
  if (!value) throw new Error(`Set JOIN_TEST_${name}; see e2e/join.md`);
  return value;
}

test("an authorized User joins without creating a local authority, and restart retains the Device", async ({
  page,
  request,
}) => {
  const origin = setting("APP_ORIGIN");
  const root = setting("ROOT");
  const port = setting("PORT");
  const helper = `https://127.0.0.1:${port}`;
  const device = setting("DEVICE");
  const admin = { Authorization: `Bearer ${setting("DEVICE_TOKEN")}` };
  const launch = () => spawn(setting("BIN"), ["serve", root, port, origin], { stdio: "inherit" });
  let child: ChildProcess | undefined;
  async function stop() {
    if (!child || child.exitCode !== null || child.signalCode !== null) return;
    const exited = once(child, "exit");
    child.kill("SIGTERM");
    await exited;
  }
  async function start() {
    child = launch();
    await expect
      .poll(async () => {
        try {
          return (
            await request.get(`${helper}/setup/join`, { headers: { Origin: origin } })
          ).status();
        } catch (error) {
          return error instanceof Error ? error.message : "HTTPS request unavailable";
        }
      })
      .toBe(200);
  }
  async function ownerRecords() {
    const response = await request.get(`${device}/devices?testProbe=1`, { headers: admin });
    expect(response.ok()).toBeTruthy();
    return (await response.json()) as {
      id: string;
      publicKey: string;
      name: string;
      state: string;
    }[];
  }
  try {
    await start();
    expect(
      (
        await request.post(`${helper}/setup/join`, { headers: { Origin: origin }, data: {} })
      ).status()
    ).toBe(401);
    expect((await request.get(`${helper}/setup/join`)).status()).toBe(403);
    expect((await request.get(`${helper}/idp/.well-known/openid-configuration`)).status()).toBe(
      404
    );
    const name = `Join browser ${Date.now()}`;
    await page.goto(`${origin}/setup/join`);
    await page.getByLabel("Local join helper").fill(helper);
    await page.getByLabel("Local setup code").fill(setting("CODE"));
    await page.getByLabel("Installation issuer").fill(setting("ISSUER"));
    await page.getByLabel("Device API", { exact: true }).fill(device);
    await page.getByLabel("Storage API", { exact: true }).fill(setting("STORAGE"));
    await page.getByLabel("Join OAuth client ID").fill(setting("CLIENT"));
    await page.getByLabel("Join Application URI").fill(setting("APPLICATION"));
    await page.getByLabel("Accepting Device public key").fill(setting("ACCEPTING_KEY"));
    await page.getByLabel("Joining Device name").fill(name);
    await page.getByLabel("Joining Device address").fill("127.0.0.1:23457");
    await page.getByRole("button", { name: "Prepare join" }).click();
    await page.getByRole("button", { name: "Sign in to Installation" }).click();
    await page.getByLabel("Username", { exact: true }).fill(setting("USERNAME"));
    await page.getByLabel("Password", { exact: true }).fill(setting("PASSWORD"));
    await page.getByRole("button", { name: "Sign in", exact: true }).click();
    await page.getByRole("button", { name: "Allow", exact: true }).click();
    await expect(page.getByText("Signed in to the Installation.")).toBeVisible();
    expect((await ownerRecords()).find((record) => record.name === name)).toBeUndefined();
    await page.getByLabel("Local setup code").fill(setting("CODE"));
    const requestPairing = page.getByRole("button", { name: "Request pairing" });
    const { JOIN_TEST_INJECTS_FAULTS } = process.env;
    if (JOIN_TEST_INJECTS_FAULTS) {
      await requestPairing.click();
      await expect(page.getByRole("alert")).toContainText(
        "The selected Installation could not complete"
      );
      expect(await (await request.get(`${device}/redirect-hits`)).json()).toBe(0);
    }
    for (const expected of JOIN_TEST_INJECTS_FAULTS ? [0, 1] : []) {
      await requestPairing.click();
      await expect(page.getByRole("alert")).toContainText("Pairing request outcome is unknown");
      expect((await ownerRecords()).filter((record) => record.name === name)).toHaveLength(
        expected
      );
      await stop();
      await start();
      await page.reload();
      await page.getByLabel("Local setup code").fill(setting("CODE"));
      await page.getByRole("button", { name: "Resume join" }).click();
    }
    await requestPairing.click();
    await expect(page.getByText("Pairing is pending.")).toBeVisible();
    const pending = (await ownerRecords()).find((record) => record.name === name);
    expect(pending?.state).toBe("pending");
    if (!pending) throw new Error("Missing pending Device");
    await expect(page.getByText(pending.id, { exact: true })).toBeVisible();
    await expect(page.getByText(pending.publicKey, { exact: true })).toBeVisible();
    const invalid = await request.post(`${device}/devices/pairings/${pending.id}/approve`, {
      headers: admin,
      data: { signature: "invalid" },
    });
    expect(invalid.status()).toBe(403);
    await stop();
    await start();
    await page.reload();
    await page.getByLabel("Local setup code").fill(setting("CODE"));
    await page.getByRole("button", { name: "Resume join" }).click();
    await expect(page.getByText("Pairing is pending.")).toBeVisible();
    await page.getByRole("button", { name: "Approve this Device" }).click();
    await expect(page.getByText("This Node joined as a Device.")).toBeVisible();
    await stop();
    await start();
    await page.reload();
    await expect(page.getByText("This Node joined as a Device.")).toBeVisible();
    await expect(page.getByText(pending.id, { exact: true })).toBeVisible();
    await expect(page.getByText(pending.publicKey, { exact: true })).toBeVisible();
    const retired = await request.post(`${helper}/setup/join`, {
      headers: { Origin: origin, Authorization: `Setup ${setting("CODE")}` },
      data: { action: "resume" },
    });
    expect(retired.status()).toBe(401);
    const approved = (await ownerRecords()).filter((record) => record.name === name);
    expect(approved).toHaveLength(1);
    expect(approved[0]).toMatchObject({
      id: pending.id,
      publicKey: pending.publicKey,
      state: "approved",
    });
  } finally {
    await stop();
  }
});
