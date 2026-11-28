import { Database } from "@ofsdk/sql";
import { expect, it } from "vitest";
import { retireLegacyReplica } from "./retire-legacy-replica";

it("reserves a missing legacy namespace so reopened SQL writers cannot initialize it", async () => {
  const name = crypto.randomUUID();
  await retireLegacyReplica(name, true);
  const database = await Database.open(name);
  const writer = database.client();
  try {
    await expect(database.initialRestoreIsEligible()).resolves.toBe(true);
    await expect(writer.executeRaw("CREATE TABLE legacy (id TEXT PRIMARY KEY)")).rejects.toThrow(
      "revision exhausted"
    );
    await expect(database.initialRestoreIsEligible()).resolves.toBe(true);
  } finally {
    writer.close();
    database.close();
    indexedDB.deleteDatabase(`ofdb:sql:${name}`);
  }
});

it("does not retire an unknown physical schema", async () => {
  const name = crypto.randomUUID();
  const physical = `ofdb:sql:${name}`;
  const db = await new Promise<IDBDatabase>((resolve, reject) => {
    const request = indexedDB.open(physical, 2);
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
    request.onupgradeneeded = () => request.result.createObjectStore("unknown");
  });
  try {
    await expect(retireLegacyReplica(name, true)).rejects.toThrow("Cannot retire");
    expect(db.objectStoreNames.contains("unknown")).toBe(true);
  } finally {
    db.close();
    indexedDB.deleteDatabase(physical);
  }
});
