import { expect, test } from "./test.js";

test("public SQL notifies only after IndexedDB confirms changed commits", async ({ page }) => {
  await page.goto(process.env.SQL_TEST_PAGE ?? "/browser-tests/index.html");
  await page.waitForFunction(() => "sql" in window);
  const result = await page.evaluate(async () => {
    const db = await window.sql.Database.open("commits");
    const writer = await window.sql.Database.open("commits");
    const client = writer.client();
    let count = 0;
    let notify: (() => void) | undefined;
    try {
      await client.executeRaw("CREATE TABLE entries (id INTEGER PRIMARY KEY)");
      const stop = db.onCommit(() => {
        count++;
        notify?.();
      });
      const next = () =>
        new Promise<void>((resolve) => {
          notify = resolve;
        });
      try {
        const rollback = await client.transaction();
        await rollback.executeRaw("INSERT INTO entries VALUES (2)");
        await rollback.rollback();
        window.faults.failNextCommit("abort");
        const aborted = await client.executeRaw("INSERT INTO entries VALUES (3)").then(
          () => false,
          () => true
        );
        await client.executeRaw("DELETE FROM entries WHERE id = 99");
        const insert = await client.transaction();
        await insert.executeRaw("INSERT INTO entries VALUES (1)");
        const hold = window.faults.holdNextCompletion("readwrite");
        const inserted = next();
        const commit = insert.commit();
        await hold.completed;
        const before = count;
        hold.release();
        await commit;
        await inserted;
        const deleted = next();
        await client.executeRaw("DELETE FROM entries WHERE id = 1");
        await deleted;
        stop();
        await client.executeRaw("INSERT INTO entries VALUES (4)");
        const { rows } = (await client.executeRaw("SELECT id FROM entries"))[0];
        return { before, count, aborted, rows };
      } finally {
        stop();
      }
    } finally {
      client.close();
      writer.close();
      db.close();
    }
  });
  expect(result).toEqual({
    before: 0,
    count: 2,
    aborted: true,
    rows: [[{ type: "integer", value: "4" }]],
  });
});
