import { afterEach, expect, it } from "vitest";
import { clearStorage } from "../../../test/utils";
import { createVault } from "../vault";
import { preparePersonalJsonImport } from "./import";

afterEach(clearStorage);

it("a retired preview cannot write and a submitted preview can write only once", async () => {
  const vault = await createVault("synthetic-import-password", "Import preview test vault");
  try {
    const text = JSON.stringify({
      items: [{ id: "source-id", type: 2, name: "Imported note", secureNote: { type: 0 } }],
    });
    const cancelled = await preparePersonalJsonImport(vault, text);
    cancelled.retire();
    await expect(cancelled.commit()).rejects.toThrow("Import preview is closed");
    expect((await vault.readItems()).items).toEqual([]);

    const ready = await preparePersonalJsonImport(vault, text);
    const commit = ready.commit();
    await expect(ready.commit()).rejects.toThrow("Import already submitted");
    await commit;
    await expect(ready.commit()).rejects.toThrow("Import preview is closed");
    expect((await vault.readItems()).items.map(({ name }) => name)).toEqual(["Imported note"]);
  } finally {
    await vault.close();
  }
}, 15000);
