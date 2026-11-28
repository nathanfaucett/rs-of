import { Database } from "@ofsdk/sql";
import { assert, expect, it } from "vitest";
import { openLegacyReplicaFence, retiredRevision } from "./retire-legacy-replica";

it("does not create a missing legacy namespace", async () => {
  const name = crypto.randomUUID();
  await expect(openLegacyReplicaFence(name)).resolves.toBeUndefined();
  expect(await indexedDB.databases()).not.toContainEqual(
    expect.objectContaining({ name: `ofdb:sql:${name}` })
  );
});

it("checks the validated revision in the same transaction as retirement", async () => {
  const name = crypto.randomUUID();
  const database = await Database.open(name);
  const writer = database.client();
  try {
    await writer.executeRaw("CREATE TABLE legacy (id TEXT PRIMARY KEY)");
    const old = await openLegacyReplicaFence(name);
    assert(old);
    try {
      await writer.executeRaw("INSERT INTO legacy (id) VALUES ('before retirement')");
      await expect(old.retire()).rejects.toThrow("validated older browser revision");
      await writer.executeRaw("INSERT INTO legacy (id) VALUES ('still writable')");
    } finally {
      old.close();
    }
    const current = await openLegacyReplicaFence(name);
    assert(current);
    try {
      await current.retire();
    } finally {
      current.close();
    }
    const retired = await openLegacyReplicaFence(name);
    retired?.close();
    expect(retired?.revision).toBe(retiredRevision);
    await expect(writer.executeRaw("DELETE FROM legacy")).rejects.toThrow("revision exhausted");
  } finally {
    writer.close();
    database.close();
    indexedDB.deleteDatabase(`ofdb:sql:${name}`);
  }
});

it("does not retire a replacement database at the same revision", async () => {
  const name = crypto.randomUUID();
  const database = await Database.open(name);
  database.close();
  const validated = await openLegacyReplicaFence(name);
  assert(validated);
  try {
    await new Promise<void>((resolve, reject) => {
      const request = indexedDB.deleteDatabase(`ofdb:sql:${name}`);
      request.onsuccess = () => resolve();
      request.onerror = () => reject(request.error);
    });
    const replacement = await Database.open(name);
    const writer = replacement.client();
    try {
      const current = await openLegacyReplicaFence(name);
      current?.close();
      expect(current?.revision).toBe(validated.revision);
      await expect(validated.retire()).rejects.toThrow("validated older browser revision");
      await writer.executeRaw("CREATE TABLE unrelated (id TEXT PRIMARY KEY)");
    } finally {
      writer.close();
      replacement.close();
    }
  } finally {
    validated.close();
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
    await expect(openLegacyReplicaFence(name)).rejects.toThrow("validated older browser revision");
    expect(db.objectStoreNames.contains("unknown")).toBe(true);
  } finally {
    db.close();
    indexedDB.deleteDatabase(physical);
  }
});
