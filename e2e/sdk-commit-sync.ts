import { expect, type Page } from "@playwright/test";
import { MAX_FRAME_BYTES, response, unframe } from "../packages/storage-sync-client/src/protocol";

export async function verifyCommitDrivenSql(
  page: Page,
  destination: { storage: string; storeId: string; token: string },
  query: () => Promise<{ status: number; body: string }>
) {
  const fixture = "/src/test/commit-sync-browser-fixture.ts";
  const socketUrl = `${destination.storage.replace(/^http/, "ws")}/sync/ws`;
  let sessions = 0;
  let hold = false;
  let release: (() => void) | undefined;
  let disconnect = false;
  await page.routeWebSocket(socketUrl, (socket) => {
    sessions++;
    const server = socket.connectToServer();
    if (disconnect) {
      disconnect = false;
      socket.onMessage(() => {
        void socket.close({ code: 1011 });
        void server.close();
      });
    } else if (hold) {
      hold = false;
      server.onMessage((message) => {
        const frame =
          typeof message === "string"
            ? undefined
            : unframe(new Uint8Array(message).buffer, MAX_FRAME_BYTES);
        const payload = frame?.[0] === 3 ? response(frame) : undefined;
        // SQL v7 Done follows the responder applying the initiator's outbound history.
        if (
          !release &&
          payload?.type === "owner" &&
          payload.data.length === 1 &&
          payload.data[0] === 8
        ) {
          release = () => socket.send(message);
        } else {
          socket.send(message);
        }
      });
    }
  });
  const execute = (sql: string) =>
    page.evaluate(async ({ fixture, sql }) => (await import(fixture)).execute(sql), {
      fixture,
      sql,
    });
  const status = () => page.evaluate(async (fixture) => (await import(fixture)).status(), fixture);
  const remoteIds = async () => {
    const result = await query();
    if (result.status !== 200) return undefined;
    return JSON.parse(result.body).results[0].rows.map(([id]: [{ value: string }]) => id.value);
  };
  try {
    await page.evaluate(
      async ({ fixture, destination }) => (await import(fixture)).start(destination),
      { fixture, destination }
    );
    await expect.poll(status).toBe("synced");
    // An actual pull wakes a follow-up, but replaying the same history must settle.
    expect(sessions).toBe(2);
    hold = true;
    await execute(
      "CREATE TABLE sdk_commit_probe (id INTEGER PRIMARY KEY); INSERT INTO sdk_commit_probe VALUES (1)"
    );
    await expect.poll(() => Boolean(release)).toBe(true);
    await expect.poll(remoteIds).toEqual(["1"]);
    await execute("INSERT INTO sdk_commit_probe VALUES (2)");
    await execute("INSERT INTO sdk_commit_probe VALUES (3)");
    await execute("DELETE FROM sdk_commit_probe WHERE id = 1");
    expect(sessions).toBe(3);
    expect(await remoteIds()).toEqual(["1"]);
    release?.();
    await expect.poll(remoteIds).toEqual(["2", "3"]);
    await expect.poll(status).toBe("synced");
    expect(sessions).toBe(4);

    await page.evaluate(async (fixture) => (await import(fixture)).rollbackInsert(), fixture);
    expect(sessions).toBe(4);
    disconnect = true;
    await execute("INSERT INTO sdk_commit_probe VALUES (4)");
    await expect.poll(status).toBe("error");
    expect(await remoteIds()).toEqual(["2", "3"]);
    await page.evaluate(() => window.dispatchEvent(new Event("online")));
    await expect.poll(remoteIds).toEqual(["2", "3", "4"]);
    await expect.poll(status).toBe("synced");

    disconnect = true;
    await execute("INSERT INTO sdk_commit_probe VALUES (5)");
    await expect.poll(status).toBe("error");
    await execute("INSERT INTO sdk_commit_probe VALUES (6)");
    await expect.poll(remoteIds).toEqual(["2", "3", "4", "5", "6"]);
    await expect.poll(status).toBe("synced");
    disconnect = true;
    await execute("INSERT INTO sdk_commit_probe VALUES (7)");
    await expect.poll(status).toBe("error");
    await page.evaluate(async (fixture) => (await import(fixture)).retry(), fixture);
    expect(await remoteIds()).toEqual(["2", "3", "4", "5", "6", "7"]);

    await page.evaluate(async (fixture) => (await import(fixture)).stop(), fixture);
    const before = sessions;
    await execute("INSERT INTO sdk_commit_probe VALUES (8)");
    expect(await execute("SELECT id FROM sdk_commit_probe WHERE id = 8")).toMatchObject([
      { rows: [[{ type: "integer", value: "8" }]] },
    ]);
    expect(await remoteIds()).toEqual(["2", "3", "4", "5", "6", "7"]);
    expect(sessions).toBe(before);
  } finally {
    await page.evaluate(async (fixture) => (await import(fixture)).close(), fixture);
  }
}
