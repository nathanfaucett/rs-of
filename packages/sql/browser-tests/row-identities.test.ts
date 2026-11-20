import type { DbValue } from "../src/index.js";
import { expect, test } from "./test.js";

test("IndexedDB retains generated UUID primary keys and rejects supplied ones", async ({
  page,
}) => {
  await page.goto(process.env.SQL_TEST_PAGE ?? "/browser-tests/row-identities.html");
  await page.waitForFunction(() => "sql" in window);

  const result = await page.evaluate(async () => {
    const { Database, DbError } = (window as unknown as { sql: typeof import("../src/index.js") })
      .sql;
    const uuid5 = "018f0f8e-7b6d-5c4a-8f12-123456789abc";
    const uuid7 = "018f0f8e-7b6d-7c4a-8f12-123456789abc";
    const uuid = (value: string): DbValue => ({ type: "uuid", value });
    const db = await Database.open("row-identities");
    const client = db.client();
    const errors = [];
    const returned = [];
    try {
      await client.executeRaw("CREATE TABLE notes (id UUID PRIMARY KEY, ref UUID NOT NULL)");
      for (const [sql, ref] of [
        ["INSERT INTO notes (ref) VALUES ($1) RETURNING id, ref", uuid5],
        ["INSERT INTO notes (id, ref) VALUES (DEFAULT, $1) RETURNING id, ref", uuid7],
      ]) {
        returned.push((await client.executeRaw(sql, [uuid(ref)]))[0].rows);
      }
      for (const id of [uuid5, uuid7]) {
        for (const insert of [
          () =>
            client.executeRaw(`INSERT INTO notes VALUES (CAST('${id}' AS UUID), $1)`, [uuid(id)]),
          () => client.executeRaw("INSERT INTO notes VALUES ($1, $2)", [uuid(id), uuid(id)]),
          () =>
            client.executeRaw("INSERT INTO notes VALUES (:id, :ref)", {
              id: uuid(id),
              ref: uuid(id),
            }),
        ]) {
          try {
            await insert();
            errors.push(null);
          } catch (error) {
            if (!(error instanceof DbError)) throw error;
            errors.push(error.message);
          }
        }
      }
      await client.executeRaw("CREATE TABLE natural_keys (name TEXT PRIMARY KEY, ref UUID)");
      await client.executeRaw(
        "CREATE TABLE composite_keys (name TEXT, ref UUID, PRIMARY KEY (name, ref))"
      );
      for (const table of ["natural_keys", "composite_keys"]) {
        await client.executeRaw(`INSERT INTO ${table} VALUES ($1, $2)`, [
          { type: "text", value: "browser" },
          uuid(uuid5),
        ]);
      }
    } finally {
      client.close();
      db.close();
    }

    const reopened = await Database.open("row-identities");
    const reader = reopened.client();
    try {
      return {
        errors,
        returned,
        persisted: (await reader.executeRaw("SELECT id, ref FROM notes"))[0].rows,
        natural: (await reader.executeRaw("SELECT name, ref FROM natural_keys"))[0].rows,
        composite: (await reader.executeRaw("SELECT name, ref FROM composite_keys"))[0].rows,
      };
    } finally {
      reader.close();
      reopened.close();
    }
  });

  const generated = {
    type: "uuid",
    value: expect.stringMatching(
      /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i
    ),
  };
  const uuid5 = { type: "uuid", value: "018f0f8e-7b6d-5c4a-8f12-123456789abc" };
  const uuid7 = { type: "uuid", value: "018f0f8e-7b6d-7c4a-8f12-123456789abc" };
  expect(result.returned).toEqual([[[generated, uuid5]], [[generated, uuid7]]]);
  expect(result.returned[0][0][0]).not.toEqual(result.returned[1][0][0]);
  expect(result.errors).toEqual(
    Array(6).fill(expect.stringContaining("UUID primary keys are generated"))
  );
  expect(result.persisted).toHaveLength(2);
  expect(result.persisted).toEqual(expect.arrayContaining(result.returned.flat()));
  const keys = [[{ type: "text", value: "browser" }, uuid5]];
  expect(result.natural).toEqual(keys);
  expect(result.composite).toEqual(keys);
});
