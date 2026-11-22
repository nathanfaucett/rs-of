import { expect, test } from "./test.js";

test.beforeEach(async ({ page }) => {
  await page.goto(process.env.SQL_TEST_PAGE ?? "/browser-tests/index.html");
  await page.waitForFunction(() => "sql" in window);
});

for (const operationKind of ["history", "legacy"] as const) {
  test(`close during deferred ${operationKind} receive defers the Rust free`, async ({
    page,
  }) => {
    const result = await page.evaluate(async (kind) => {
      const wasm = await import("/pkg/ofdb_wasm.js");
      const originalFree = wasm.SqlDatabase.prototype.free;
      let freeCalls = 0;
      let closeError: string | undefined;
      try {
        wasm.SqlDatabase.prototype.free = function (this: any) {
          freeCalls++;
          return originalFree.call(this);
        };
        const {
          Database,
          JsPushSyncTransport,
          JsSqlSyncTransport,
          PushSynchronizeConfig,
        } = (window as any).sql;
        const database = await Database.open(`close-during-${kind}`);
        let started!: () => void;
        const receiveStarted = new Promise<void>((resolve) => {
          started = resolve;
        });
        let rejectReceive!: (error: Error) => void;
        let closed = false;
        let closeFailure: Error | undefined;
        const transport = {
          send: async () => undefined,
          receive: () => {
            started();
            if (closed) return Promise.reject(closeFailure);
            return new Promise<Uint8Array>((_, reject) => {
              rejectReceive = reject;
            });
          },
        };
        const operation =
          kind === "history"
            ? database.synchronizeHistory(new JsSqlSyncTransport(transport))
            : database.synchronize(
                new JsPushSyncTransport(transport),
                new PushSynchronizeConfig(),
              );
        const settled = operation.then(
          () => "unexpected success",
          (error: unknown) => String(error),
        );
        await receiveStarted;
        try {
          database.close();
        } catch (error) {
          closeError = String(error);
        }
        const freeBeforeSettlement = freeCalls;
        closed = true;
        closeFailure = new Error("deferred-close");
        rejectReceive(closeFailure);
        const syncError = await settled;
        const freeAfterSettlement = freeCalls;
        database.close();
        const freeAfterSecondClose = freeCalls;
        return {
          closeError,
          syncError,
          freeBeforeSettlement,
          freeAfterSettlement,
          freeAfterSecondClose,
        };
      } finally {
        wasm.SqlDatabase.prototype.free = originalFree;
      }
    }, operationKind);

    expect(result.closeError).toBeUndefined();
    expect(result.syncError).toContain("deferred-close");
    expect(result.freeBeforeSettlement).toBe(0);
    expect(result.freeAfterSettlement).toBe(1);
    expect(result.freeAfterSecondClose).toBe(1);
  });
}

test("history rejects an oversized or non-byte receive without exposing payloads", async ({
  page,
}) => {
  const result = await page.evaluate(async () => {
    const { Database, JsSqlSyncTransport } = (window as any).sql;
    const errors: string[] = [];
    for (const received of [
      new Uint8Array(1024 * 1024 + 1).fill(65),
      { payload: "not bytes" },
    ]) {
      const database = await Database.open("oversized-history-frame");
      try {
        await database.synchronizeHistory(
          new JsSqlSyncTransport({
            send: async () => undefined,
            receive: async () => received,
          }),
        );
        errors.push("unexpected success");
      } catch (error: any) {
        errors.push(error.message);
      } finally {
        database.close();
      }
    }
    return errors;
  });

  expect(result).toEqual([
    "transport error: received SQL history frame exceeds 1048576 bytes",
    "transport error: received SQL history frame must be a Uint8Array",
  ]);
});
