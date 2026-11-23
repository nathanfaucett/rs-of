import { expect, test } from "./test.js";

test.beforeEach(async ({ page }) => {
  await page.goto(process.env.SQL_TEST_PAGE ?? "/browser-tests/index.html");
  await page.waitForFunction(() => "sql" in window);
});

test("legacy push synchronization uses IndexedDB committed changes and batched acknowledgments", async ({
  page,
}) => {
  const result = await page.evaluate(async () => {
    const {
      Database,
      JsPushSyncTransport,
      JsSqlSyncTransport,
      PushConfig,
      SqlSessionConfig,
      SqlHistoryRole,
      SqlSyncRole,
    } = window.sql;
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
    const transports = endpoints.map((endpoint) => ({
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

    const a = await Database.open("legacy-a");
    const b = await Database.open("legacy-b");
    const aClient = a.client();
    const bClient = b.client();
    try {
      await aClient.executeRaw("CREATE TABLE entries (id INTEGER PRIMARY KEY, value TEXT)");
      await bClient.executeRaw("CREATE TABLE entries (id INTEGER PRIMARY KEY, value TEXT)");
      await aClient.executeRaw("INSERT INTO entries VALUES (1, 'initial')");
      await Promise.all([
        a.synchronizeHistory(new JsSqlSyncTransport(transports[0]), SqlHistoryRole.Initiator),
        b.synchronizeHistory(new JsSqlSyncTransport(transports[1]), SqlHistoryRole.Responder),
      ]);
      await aClient.executeRaw("UPDATE entries SET value = 'committed' WHERE id = 1");
      const outcomes = await Promise.allSettled([
        a.synchronize(
          new JsPushSyncTransport(transports[0]),
          Object.assign(new SqlSessionConfig(), {
            protocol: Object.assign(new PushConfig(), { immediateAck: false }),
          })
        ),
        b.synchronize(
          new JsPushSyncTransport(transports[1]),
          Object.assign(new SqlSessionConfig(), {
            role: SqlSyncRole.Acceptor,
            protocol: Object.assign(new PushConfig(), { immediateAck: false }),
          })
        ),
      ]);
      const errors = outcomes.flatMap((outcome) =>
        outcome.status === "rejected" ? [String(outcome.reason)] : []
      );
      const rows = errors.length ? [] : await bClient.executeRaw("SELECT id, value FROM entries");
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

test("rejected IndexedDB commits never enter immediate or later legacy change streams", async ({
  page,
}) => {
  const result = await page.evaluate(async () => {
    const wasm = await import("/pkg/ofdb_wasm.js");
    await wasm.default();
    const database = await wasm.SqlDatabase.open("commit-signal");
    const client = database.client();
    const staleDatabase = await wasm.SqlDatabase.open("commit-signal");
    const staleClient = staleDatabase.client();
    const execute = (
      handle: { execute(sql: string, params: undefined): Promise<string> },
      sql: string
    ) => handle.execute(sql, undefined);
    const subscription = staleDatabase.sync_changes();
    try {
      await execute(client, "CREATE TABLE entries (id INTEGER PRIMARY KEY, value TEXT)");
      await execute(client, "INSERT INTO entries VALUES (1, 'initial')");

      const staleTransaction = await staleClient.transaction();
      await staleTransaction.execute(
        "UPDATE entries SET value = 'rejected' WHERE id = 1",
        undefined
      );
      await execute(client, "UPDATE entries SET value = 'durable' WHERE id = 1");
      let commitRejected = false;
      try {
        await staleTransaction.commit();
      } catch {
        commitRejected = true;
      }

      await execute(staleClient, "UPDATE entries SET value = 'accepted' WHERE id = 1");
      const acceptedEvent = await subscription.next_event();
      await execute(staleClient, "UPDATE entries SET value = 'latest' WHERE id = 1");
      const laterEvent = await subscription.next_event();
      return {
        commitRejected,
        acceptedBytes: Array.from(acceptedEvent?.blob ?? []),
        laterBytes: Array.from(laterEvent?.blob ?? []),
      };
    } finally {
      subscription.free();
      staleClient.free();
      client.free();
      staleDatabase.free();
      database.free();
    }
  });

  const decode = (bytes: number[]) => new TextDecoder().decode(Uint8Array.from(bytes));
  expect(result.commitRejected).toBe(true);
  expect(decode(result.acceptedBytes)).toContain("accepted");
  expect(decode(result.acceptedBytes)).not.toContain("rejected");
  expect(decode(result.laterBytes)).toContain("latest");
  expect(decode(result.laterBytes)).not.toContain("rejected");
});
