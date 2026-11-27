import { openSharedDatabase } from "$lib/vault/storage/migrate-to-shared";

export async function writeCompetingBinding(owner: string) {
  const database = await openSharedDatabase();
  try {
    const tag = new TextEncoder().encode(JSON.stringify([owner, "competing-resource", false]));
    if (!(await database.replaceInitialRestoreTag(undefined, tag)))
      throw new Error("Fixture write lost the race");
  } finally {
    database.close();
  }
}

export async function removeCompetingBinding() {
  const { name } =
    (await indexedDB.databases()).find(({ name }) => name?.endsWith(":password-manager-v1")) ?? {};
  if (!name) throw new Error("Missing local vault database");
  await new Promise<void>((resolve, reject) => {
    const open = indexedDB.open(name);
    open.onerror = () => reject(open.error);
    open.onsuccess = () => {
      const database = open.result;
      const tx = database.transaction("records", "readwrite");
      tx.oncomplete = () => {
        database.close();
        resolve();
      };
      tx.onabort = () => {
        database.close();
        reject(tx.error);
      };
      tx.objectStore("records").delete(["ofdb:local:initial-restore", new Uint8Array()]);
    };
  });
}
