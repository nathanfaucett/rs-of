export type { Entry } from "./client.js";
export { Client } from "./client.js";
export { Database } from "./database.js";
export type { DbErrorKind } from "./error.js";
export { DbError, KvError as Error } from "./error.js";
export { Transaction } from "./transaction.js";
export type { DbValue } from "./value.js";

// Sync exports - import to ensure they're included in bundle
import {
  JsChangeEvent,
  JsKvSyncError,
  JsPushSyncTransport,
  KvChangeSubscription,
  KvSyncRole,
  PushConfig,
  PushSynchronizeConfig,
  PushSynchronizeResult,
} from "./database.js";

export {
  JsChangeEvent,
  JsKvSyncError,
  JsPushSyncTransport,
  KvChangeSubscription,
  KvSyncRole,
  PushConfig,
  PushSynchronizeConfig,
  PushSynchronizeResult,
};
