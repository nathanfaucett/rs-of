import { test as base, expect } from "@playwright/test";

interface CompletionHold {
  completed: Promise<void>;
  release(): void;
}

declare global {
  interface Window {
    sql: typeof import("../src/index.js");
    makeSqlWorker(): Worker;
    faults: {
      denyOpen(): void;
      malformed(name: string, kind: string, format: string): Promise<void>;
      sentinel(name: string, kind: string): Promise<string>;
      failNextCommit(mode: string): void;
      holdNextCompletion(stage: string): CompletionHold;
    };
    pendingTransaction: import("../src/index.js").Transaction;
    pendingCommit: Promise<void>;
  }
}

export { expect };
export const test = base.extend({
  page: async ({ page }, use) => {
    await use(page);
    await page.evaluate(async () => {
      for (const { name } of await indexedDB.databases()) {
        if (!name?.startsWith("ofdb:sql:")) continue;
        await new Promise<void>((resolve, reject) => {
          const request = indexedDB.deleteDatabase(name);
          request.onsuccess = () => resolve();
          request.onerror = () => reject(request.error);
          request.onblocked = () =>
            reject(new Error("owned SQL handles leaked an IndexedDB connection"));
        });
      }
    });
  },
});
