import { expect, test } from "./test.js";

test.beforeEach(async ({ page }) => {
  await page.goto(process.env.SQL_TEST_PAGE ?? "/browser-tests/index.html");
  await page.waitForFunction(() => "sql" in window);
});

test("legacy push synchronization uses IndexedDB committed changes", async ({
  page,
}) => {
  const result = await page.evaluate(async () => {
    const {
      Database,
      JsPushSyncTransport,
      JsSqlHistoryTransport,
      PushSynchronizeConfig,
      SqlHistoryRole,
    } = (window as any).sql;
    const endpoints = [0, 1].map(() => ({
      queue: [] as Uint8Array[],
      waiting: undefined as ((data: Uint8Array) => void) | undefined,
      peer: undefined as any,
    }));
    endpoints[0].peer = endpoints[1];
    endpoints[1].peer = endpoints[0];
    const transports = endpoints.map((endpoint) => ({
      send(data: Uint8Array) {
        const copy = new Uint8Array(data);
        if (endpoint.peer.waiting) {
          endpoint.peer.waiting(copy);
          endpoint.peer.waiting = undefined;
        } else {
          endpoint.peer.queue.push(copy);
        }
        return Promise.resolve();
      },
      receive() {
        const data = endpoint.queue.shift();
        if (data) return Promise.resolve(data);
        return new Promise<Uint8Array>((resolve) => {
          endpoint.waiting = resolve;
        });
      },
    }));

    const a = await Database.open("legacy-a");
    const b = await Database.open("legacy-b");
    const aClient = a.client();
    const bClient = b.client();
    try {
      await aClient.executeRaw(
        "CREATE TABLE entries (id INTEGER PRIMARY KEY, value TEXT)",
      );
      await bClient.executeRaw(
        "CREATE TABLE entries (id INTEGER PRIMARY KEY, value TEXT)",
      );
      await aClient.executeRaw("INSERT INTO entries VALUES (1, 'initial')");
      await Promise.all([
        a.synchronizeHistory(
          new JsSqlHistoryTransport(transports[0]),
          SqlHistoryRole.Initiator,
        ),
        b.synchronizeHistory(
          new JsSqlHistoryTransport(transports[1]),
          SqlHistoryRole.Responder,
        ),
      ]);
      await aClient.executeRaw(
        "UPDATE entries SET value = 'committed' WHERE id = 1",
      );
      const outcomes = await Promise.allSettled([
        a.synchronize(
          new JsPushSyncTransport(transports[0]),
          new PushSynchronizeConfig(),
        ),
        b.synchronize(
          new JsPushSyncTransport(transports[1]),
          new PushSynchronizeConfig(),
        ),
      ]);
      const errors = outcomes.flatMap((outcome) =>
        outcome.status === "rejected" ? [String(outcome.reason)] : [],
      );
      const rows = errors.length
        ? []
        : await bClient.executeRaw("SELECT id, value FROM entries");
      return { errors, rows };
    } finally {
      aClient.close();
      bClient.close();
      a.close();
      b.close();
    }
  });

  expect(result.errors).toEqual([]);
  expect(result.rows[0].rows).toEqual([
    [
      { type: "integer", value: "1" },
      { type: "text", value: "committed" },
    ],
  ]);
});
