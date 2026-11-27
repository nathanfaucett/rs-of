import { expect, test } from "./test.js";

test.beforeEach(async ({ page }) => {
  await page.goto(process.env.SQL_TEST_PAGE ?? "/browser-tests/index.html");
  await page.waitForFunction(() => "sql" in window);
});

test("public history v7 synchronizes both IndexedDB peers without duplicating rows", async ({
  page,
}) => {
  const result = await page.evaluate(async () => {
    const { Database, JsSqlSyncTransport, SqlHistoryRole } = window.sql;

    function transportPair() {
      interface TransportEndpoint {
        queue: Uint8Array[];
        waiting: ((data: Uint8Array) => void) | undefined;
        peer: TransportEndpoint | undefined;
      }
      const endpoints: TransportEndpoint[] = [0, 1].map(() => ({
        queue: [],
        waiting: undefined,
        peer: undefined,
      }));
      const [first, second] = endpoints;
      if (!first || !second) throw new Error("transport pair is required");
      first.peer = second;
      second.peer = first;
      return endpoints.map((endpoint) => ({
        send(data: Uint8Array) {
          const peer = endpoint.peer;
          if (!peer) return Promise.reject(new Error("transport peer is not connected"));
          const copy = new Uint8Array(data);
          if (peer.waiting) {
            peer.waiting(copy);
            peer.waiting = undefined;
          } else {
            peer.queue.push(copy);
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
    const read = async (client: import("../src/index.js").Client) => {
      const results = await client.executeRaw("SELECT id, value FROM entries ORDER BY id");
      return results[0].rows.map((row) =>
        row.map((value) => ("value" in value ? value.value : undefined))
      );
    };
    const sync = async () => {
      const [aTransport, bTransport] = transportPair();
      await Promise.all([
        a.synchronizeHistory(new JsSqlSyncTransport(aTransport), SqlHistoryRole.Initiator),
        b.synchronizeHistory(new JsSqlSyncTransport(bTransport), SqlHistoryRole.Responder),
      ]);
    };

    try {
      await aClient.executeRaw("CREATE TABLE entries (id INTEGER PRIMARY KEY, value TEXT)");
      await bClient.executeRaw("CREATE TABLE entries (id INTEGER PRIMARY KEY, value TEXT)");
      await aClient.executeRaw("INSERT INTO entries VALUES (1, 'from-a')");
      await bClient.executeRaw("INSERT INTO entries VALUES (2, 'from-b')");
      const savedTag = await a.replaceInitialRestoreTag(undefined, Uint8Array.of(3, 4));
      const replacedExisting = await a.replaceInitialRestoreTag(undefined, Uint8Array.of(5));
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
      const retainedTag = Array.from((await a.initialRestoreTag()) ?? []);
      const peerTag = (await b.initialRestoreTag()) ?? null;

      await aClient.executeRaw("UPDATE entries SET value = 'updated-a' WHERE id = 1");
      await bClient.executeRaw("UPDATE entries SET value = 'updated-b' WHERE id = 2");
      await sync();
      const updated = [await read(aClient), await read(bClient)];

      await aClient.executeRaw("DELETE FROM entries WHERE id = 2");
      await bClient.executeRaw("DELETE FROM entries WHERE id = 1");
      await sync();
      const deleted = [await read(aClient), await read(bClient)];
      await sync();
      const repeatedDelete = [await read(aClient), await read(bClient)];
      return {
        first,
        repeated,
        reopened,
        updated,
        deleted,
        repeatedDelete,
        savedTag,
        replacedExisting,
        retainedTag,
        peerTag,
      };
    } finally {
      close();
    }
  });

  const expected = [
    ["1", "from-a"],
    ["2", "from-b"],
  ];
  expect(result.savedTag).toBe(true);
  expect(result.replacedExisting).toBe(false);
  expect(result.retainedTag).toEqual([3, 4]);
  expect(result.peerTag).toBeNull();
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

test("restore refuses retained history from another IndexedDB handle during exchange", async ({
  page,
}) => {
  const result = await page.evaluate(async () => {
    const { Database, JsSqlSyncTransport, SqlHistoryRole } = window.sql;
    const source = await Database.open("restore-source");
    const destination = await Database.open("restore-destination");
    const otherHandle = await Database.open("restore-destination");
    const sourceClient = source.client();
    const writer = otherHandle.client();
    const channel = new MessageChannel();
    function endpoint(port: MessagePort) {
      const queued: Uint8Array[] = [];
      let waiting: ((bytes: Uint8Array) => void) | undefined;
      port.onmessage = ({ data }: MessageEvent<Uint8Array>) => {
        if (waiting) {
          const resolve = waiting;
          waiting = undefined;
          resolve(data);
        } else queued.push(data);
      };
      return {
        async send(bytes: Uint8Array) {
          port.postMessage(bytes);
        },
        async receive() {
          return (
            queued.shift() ??
            new Promise<Uint8Array>((resolve) => {
              waiting = resolve;
            })
          );
        },
      };
    }
    const pull = endpoint(channel.port1);
    const serve = endpoint(channel.port2);
    let first = true;
    try {
      await sourceClient.executeRaw("CREATE TABLE remote_items (id INTEGER PRIMARY KEY)");
      const [restored] = await Promise.allSettled([
        destination.restoreHistory(
          new JsSqlSyncTransport({
            ...pull,
            async send(bytes: Uint8Array) {
              if (first) {
                first = false;
                await writer.executeRaw("CREATE TABLE local_items (id INTEGER PRIMARY KEY)");
                await writer.executeRaw("DROP TABLE local_items");
              }
              await pull.send(bytes);
            },
          }),
          new AbortController().signal,
          Uint8Array.of(1)
        ),
        source.synchronizeHistory(new JsSqlSyncTransport(serve), SqlHistoryRole.Responder),
      ]);
      const tables = await writer.execute("SELECT name FROM tables", (row) => row.name);
      return {
        error: restored.status === "rejected" ? String(restored.reason) : null,
        tables,
        eligible: await destination.initialRestoreIsEligible(),
        tag: (await destination.initialRestoreTag()) ?? null,
      };
    } finally {
      channel.port1.close();
      channel.port2.close();
      writer.close();
      sourceClient.close();
      otherHandle.close();
      destination.close();
      source.close();
    }
  });
  expect(result.error).toContain("destination already has retained history");
  expect(result.tables).toEqual([]);
  expect(result.eligible).toBe(false);
  expect(result.tag).toBeNull();
});

for (const competing of ["vault write", "destination", "detach"]) {
  test(`local replica tag CAS preserves a competing ${competing}`, async ({ page }) => {
    const result = await page.evaluate(async (competing) => {
      const { Database } = window.sql;
      const db = await Database.open("tag-race");
      const other = await Database.open("tag-race");
      const writer = other.client();
      try {
        await writer.executeRaw("CREATE TABLE vaults (id INTEGER PRIMARY KEY)");
        const expected = competing === "detach" ? Uint8Array.of(1) : undefined;
        if (expected) await other.replaceInitialRestoreTag(undefined, expected);
        const hold = window.faults.holdNextCompletion("readonly");
        const pending = db.replaceInitialRestoreTag(expected, Uint8Array.of(2)).then(
          (saved) => saved,
          (error: unknown) => String(error)
        );
        try {
          await hold.completed;
          if (competing === "vault write") await writer.executeRaw("INSERT INTO vaults VALUES (1)");
          else await other.replaceInitialRestoreTag(expected, Uint8Array.of(3));
        } finally {
          hold.release();
        }
        const saved = await pending;
        const tag = Array.from((await db.initialRestoreTag()) ?? []);
        const { rows } = (await writer.executeRaw("SELECT id FROM vaults"))[0];
        return { saved, tag, rows };
      } finally {
        writer.close();
        other.close();
        db.close();
      }
    }, competing);
    expect(result).toEqual(
      competing === "vault write"
        ? { saved: true, tag: [2], rows: [[{ type: "integer", value: "1" }]] }
        : { saved: false, tag: [3], rows: [] }
    );
  });
}
