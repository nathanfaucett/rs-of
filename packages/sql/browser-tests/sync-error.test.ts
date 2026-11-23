import { expect, test } from "./test.js";

test.beforeEach(async ({ page }) => {
  await page.goto(process.env.SQL_TEST_PAGE ?? "/browser-tests/index.html");
  await page.waitForFunction(() => "sql" in window);
});

test("SQL errors retain a WASM object's message and cause without logging payloads", async ({
  page,
}) => {
  const result = await page.evaluate(async () => {
    const { Database, Error: SqlError, JsSqlSyncTransport } = window.sql;
    const database = await Database.open("sync-error");
    const logged: string[] = [];
    const original = {
      log: console.log,
      warn: console.warn,
      error: console.error,
    };
    console.log = (...values) => logged.push(values.join(" "));
    console.warn = (...values) => logged.push(values.join(" "));
    console.error = (...values) => logged.push(values.join(" "));
    try {
      await database.synchronizeHistory(
        new JsSqlSyncTransport({
          send: async () => {
            throw { message: "representative-transport-error" };
          },
          receive: async () => new Uint8Array(),
        })
      );
      return { unexpectedSuccess: true };
    } catch (error) {
      return {
        typed: error instanceof SqlError,
        message: error.message,
        causeMessage: error.cause?.message,
        causeIsObject: typeof error.cause === "object" && error.cause !== null,
        logged,
      };
    } finally {
      console.log = original.log;
      console.warn = original.warn;
      console.error = original.error;
      database.close();
    }
  });

  expect(result).toEqual({
    typed: true,
    message: "transport error: transport send failed: representative-transport-error",
    causeMessage: "transport error: transport send failed: representative-transport-error",
    causeIsObject: true,
    logged: [],
  });
});
