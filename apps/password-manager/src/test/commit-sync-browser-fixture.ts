import { type Client, Database } from "@ofsdk/sql";
import { type SqlResourceSync, startSqlResourceSync } from "@ofsdk/sync-sdk/browser";

let database: Database | undefined;
let client: Client | undefined;
let sync: SqlResourceSync | undefined;
let state = "starting";
let cancelNextCheck = false;

export async function start({
  storage,
  storeId,
  token,
}: {
  storage: string;
  storeId: string;
  token: string;
}) {
  database = await Database.open("sdk-commit-probe");
  client = database.client();
  const exchange: SqlResourceSync = startSqlResourceSync(
    database,
    {
      storeId,
      replicaId: crypto.randomUUID(),
      creationEpoch: 0n,
      schemaFingerprint: new Uint8Array(32),
    },
    {
      storageApiUrl: storage,
      tokenProvider: () => token,
      checkActive: () => {
        if (cancelNextCheck) {
          cancelNextCheck = false;
          void exchange.close().then(() => {
            state = "cancelled";
          });
        }
        return true;
      },
      onStatus: (next) => {
        state = next.status;
      },
    }
  );
  sync = exchange;
}

export const status = () => state;
export const armCancellation = () => {
  cancelNextCheck = true;
};
export const initialRestoreIsEligible = () => database?.initialRestoreIsEligible();
function writer() {
  if (!client) throw new Error("Commit probe is not open");
  return client;
}
export const execute = (sql: string) => writer().executeRaw(sql);
export const stop = () => sync?.close();
export const retry = () => sync?.sync();
export async function rollbackInsert() {
  const tx = await writer().transaction();
  await tx.executeRaw("INSERT INTO sdk_commit_probe VALUES (99)");
  await tx.rollback();
}
export async function close() {
  await sync?.close();
  client?.close();
  database?.close();
}
