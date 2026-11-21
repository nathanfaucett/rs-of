import { expect, test } from "./test.js";

test.beforeEach(async ({ page }) => {
  await page.goto(process.env.SQL_TEST_PAGE ?? "/browser-tests/index.html");
  await page.waitForFunction(() => "sql" in window);
});

test("public history v7 synchronizes both IndexedDB peers without duplicating rows", async ({
  page,
}) => {
  const result = await page.evaluate(async () => {
    const {
      Database,
      JsSqlSyncTransport,
      SqlHistoryRole,
    } = (window as any).sql;

    function transportPair() {
      const endpoints = [0, 1].map(() => ({
        queue: [] as Uint8Array[],
        waiting: undefined as ((data: Uint8Array) => void) | undefined,
        peer: undefined as any,
      }));
      endpoints[0].peer = endpoints[1];
      endpoints[1].peer = endpoints[0];
      return endpoints.map((endpoint) => ({
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
    }

    let a = await Database.open("history-a");
    let b = await Database.open("history-b");
    let aClient = a.client();
    let bClient = b.client();
    const close = () => {
      aClient.close();
      bClient.close();
      a.close();
      b.close();
    };
    const read = async (client: any) => {
      const results = await client.executeRaw(
        "SELECT id, value FROM entries ORDER BY id",
      );
      return results[0].rows.map((row: any[]) =>
        row.map((value) => value.value),
      );
    };
    const sync = async () => {
      const [aTransport, bTransport] = transportPair();
      await Promise.all([
        a.synchronizeHistory(
          new JsSqlSyncTransport(aTransport),
          SqlHistoryRole.Initiator,
        ),
        b.synchronizeHistory(
          new JsSqlSyncTransport(bTransport),
          SqlHistoryRole.Responder,
        ),
      ]);
    };

    try {
      await aClient.executeRaw(
        "CREATE TABLE entries (id INTEGER PRIMARY KEY, value TEXT)",
      );
      await bClient.executeRaw(
        "CREATE TABLE entries (id INTEGER PRIMARY KEY, value TEXT)",
      );
      await aClient.executeRaw("INSERT INTO entries VALUES (1, 'from-a')");
      await bClient.executeRaw("INSERT INTO entries VALUES (2, 'from-b')");
      await sync();
      const first = [await read(aClient), await read(bClient)];
      await sync();
      const repeated = [await read(aClient), await read(bClient)];

      close();
      a = await Database.open("history-a");
      b = await Database.open("history-b");
      aClient = a.client();
      bClient = b.client();
      const reopened = [await read(aClient), await read(bClient)];

      await aClient.executeRaw(
        "UPDATE entries SET value = 'updated-a' WHERE id = 1",
      );
      await bClient.executeRaw(
        "UPDATE entries SET value = 'updated-b' WHERE id = 2",
      );
      await sync();
      const updated = [await read(aClient), await read(bClient)];

      await aClient.executeRaw("DELETE FROM entries WHERE id = 2");
      await bClient.executeRaw("DELETE FROM entries WHERE id = 1");
      await sync();
      const deleted = [await read(aClient), await read(bClient)];
      await sync();
      const repeatedDelete = [await read(aClient), await read(bClient)];
      return { first, repeated, reopened, updated, deleted, repeatedDelete };
    } finally {
      close();
    }
  });

  const expected = [
    ["1", "from-a"],
    ["2", "from-b"],
  ];
  expect(result.first).toEqual([expected, expected]);
  expect(result.repeated).toEqual([expected, expected]);
  expect(result.reopened).toEqual([expected, expected]);
  const updated = [
    ["1", "updated-a"],
    ["2", "updated-b"],
  ];
  expect(result.updated).toEqual([updated, updated]);
  expect(result.deleted).toEqual([[], []]);
  expect(result.repeatedDelete).toEqual([[], []]);
});
