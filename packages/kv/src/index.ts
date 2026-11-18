export type { Entry } from "./client.js";
export { Client } from "./client.js";
export { Database } from "./database.js";
export type { DbErrorKind } from "./error.js";
export { DbError, KvError as Error } from "./error.js";
export { Transaction } from "./transaction.js";
export type { DbValue } from "./value.js";

// Sync exports - import to ensure they're included in bundle
import {
  JsPushSyncTransport,
  PushConfig,
  PushSynchronizeConfig,
  PushSynchronizeResult,
  KvChangeSubscription,
  JsChangeEvent,
  JsKvSyncError,
} from "./database.js";

export {
  JsPushSyncTransport,
  PushConfig,
  PushSynchronizeConfig,
  PushSynchronizeResult,
  KvChangeSubscription,
  JsChangeEvent,
  JsKvSyncError,
};
